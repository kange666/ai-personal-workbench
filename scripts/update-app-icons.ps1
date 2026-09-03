param(
    # 默认沿用现有品牌图案，只修复圆角外部的白底，不重新绘制标志。
    [string]$Source = ''
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
if ([string]::IsNullOrWhiteSpace($Source)) {
    $Source = Join-Path $repoRoot 'src/assets/app-logo.png'
}
$sourcePath = (Resolve-Path -LiteralPath $Source).Path
$workDirectory = Join-Path ([IO.Path]::GetTempPath()) ('astrion-icons-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $workDirectory | Out-Null
Copy-Item -LiteralPath $sourcePath -Destination (Join-Path $workDirectory 'original.png')
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;

public static class AstrionIconTransparency {
    public static int ClearIcoCorners(string path) {
        byte[] ico = File.ReadAllBytes(path);
        int count = BitConverter.ToUInt16(ico, 4), changed = 0;
        var frames = new byte[count][];
        for (int i = 0; i < count; i++) {
            int entry = 6 + i * 16;
            int length = (int)BitConverter.ToUInt32(ico, entry + 8);
            int offset = (int)BitConverter.ToUInt32(ico, entry + 12);
            frames[i] = new byte[length];
            Array.Copy(ico, offset, frames[i], 0, length);
            // Tauri 输出 PNG 格式的 ICO 帧；仅修正缩小后重新产生的角点覆盖率。
            using (var input = new MemoryStream(frames[i]))
            using (var frame = new Bitmap(input)) {
                bool dirty = false;
                foreach (var p in new [] { new Point(0,0), new Point(frame.Width-1,0),
                        new Point(0,frame.Height-1), new Point(frame.Width-1,frame.Height-1) }) {
                    if (frame.GetPixel(p.X,p.Y).A == 0) continue;
                    frame.SetPixel(p.X,p.Y,Color.FromArgb(0,0,0,0));
                    dirty = true;
                    changed++;
                }
                if (dirty) {
                    using (var output = new MemoryStream()) {
                        frame.Save(output, ImageFormat.Png);
                        frames[i] = output.ToArray();
                    }
                }
            }
        }
        using (var output = new MemoryStream())
        using (var writer = new BinaryWriter(output)) {
            writer.Write(ico, 0, 6);
            int offset = 6 + count * 16;
            for (int i = 0; i < count; i++) {
                writer.Write(ico, 6 + i * 16, 8);
                writer.Write(frames[i].Length);
                writer.Write(offset);
                offset += frames[i].Length;
            }
            foreach (var frame in frames) writer.Write(frame);
            writer.Flush();
            File.WriteAllBytes(path, output.ToArray());
        }
        return changed;
    }

    public static int Clean(string input, string output) {
        using (var source = new Bitmap(input))
        using (var result = new Bitmap(source.Width, source.Height, PixelFormat.Format32bppArgb)) {
            int width = source.Width, height = source.Height;
            if (width != height || width < 256)
                throw new ArgumentException("Expected the square, full-resolution brand icon.");
            for (int y = 0; y < height; y++)
                for (int x = 0; x < width; x++)
                    result.SetPixel(x, y, source.GetPixel(x, y));

            // 仅允许修改四角的外部连通区域，内部白色星芒和蓝紫色图案绝不参与。
            int corner = width / 5, changed = 0;
            var visited = new bool[width * height];
            var queue = new Queue<Point>();
            queue.Enqueue(new Point(0, 0));
            queue.Enqueue(new Point(width - 1, 0));
            queue.Enqueue(new Point(0, height - 1));
            queue.Enqueue(new Point(width - 1, height - 1));
            while (queue.Count > 0) {
                Point p = queue.Dequeue();
                if (p.X < 0 || p.Y < 0 || p.X >= width || p.Y >= height) continue;
                if (!((p.X < corner || p.X >= width - corner) &&
                      (p.Y < corner || p.Y >= height - corner))) continue;
                int index = p.Y * width + p.X;
                if (visited[index]) continue;
                visited[index] = true;
                Color original = source.GetPixel(p.X, p.Y);
                // 已经透明的素材直接保留；深色瓷片是连通区域的边界。
                if (original.A != 255 || original.R <= 22 || original.G <= 22 || original.B <= 22)
                    continue;
                double brightness = (original.R + original.G + original.B) / 3.0;
                Color cleaned = Color.FromArgb(0, 0, 0, 0);
                if (brightness < 248) {
                    int dx = p.X < corner ? 1 : -1, dy = p.Y < corner ? 1 : -1;
                    Color tile = Color.FromArgb(14, 18, 30);
                    for (int step = 2; step <= 24; step++) {
                        Color sample = source.GetPixel(p.X + dx * step, p.Y + dy * step);
                        if (sample.R < 22 && sample.G < 24 && sample.B < 38) {
                            tile = sample;
                            break;
                        }
                    }
                    // 原图圆角边缘经过白底混合：恢复透明度并去除白色污染，避免白边。
                    double tileBrightness = (tile.R + tile.G + tile.B) / 3.0;
                    int alpha = (int)Math.Round(255 * (254 - brightness) / (254 - tileBrightness));
                    alpha = Math.Max(0, Math.Min(255, alpha));
                    cleaned = Color.FromArgb(alpha, tile.R, tile.G, tile.B);
                }
                result.SetPixel(p.X, p.Y, cleaned);
                changed++;
                queue.Enqueue(new Point(p.X - 1, p.Y));
                queue.Enqueue(new Point(p.X + 1, p.Y));
                queue.Enqueue(new Point(p.X, p.Y - 1));
                queue.Enqueue(new Point(p.X, p.Y + 1));
            }
            foreach (var p in new [] {new Point(0,0), new Point(width-1,0),
                                      new Point(0,height-1), new Point(width-1,height-1)}) {
                if (result.GetPixel(p.X, p.Y).A != 0)
                    throw new InvalidOperationException("A corner is still opaque.");
            }
            // 逐像素检查主体完全不变，而不是仅检查中心的一个点。
            for (int y = 0; y < height; y++)
                for (int x = 0; x < width; x++)
                    if ((x >= corner && x < width-corner) || (y >= corner && y < height-corner))
                        if (source.GetPixel(x,y).ToArgb() != result.GetPixel(x,y).ToArgb())
                            throw new InvalidOperationException("Interior artwork changed.");
            result.Save(output, ImageFormat.Png);
            return changed;
        }
    }
}
'@

$cleanLogo = Join-Path $workDirectory 'app-logo.png'
$changedPixels = [AstrionIconTransparency]::Clean($sourcePath, $cleanLogo)
$generatedDirectory = Join-Path $workDirectory 'generated'
Push-Location $repoRoot
try {
    # 只使用仓库已经安装的 Tauri CLI，不下载其他生成工具。
    & (Join-Path $repoRoot 'node_modules/.bin/tauri.cmd') icon $cleanLogo --output $generatedDirectory
    if ($LASTEXITCODE -ne 0) { throw 'Tauri icon generation failed.' }
} finally { Pop-Location }

$fixedIcoCorners = [AstrionIconTransparency]::ClearIcoCorners((Join-Path $generatedDirectory 'icon.ico'))

# 当前仅发布 Windows NSIS，保持无关的 macOS、移动端及 Store 素材不变。
$iconNames = @('icon.ico', 'icon.png', '32x32.png', '64x64.png', '128x128.png', '128x128@2x.png')
foreach ($name in $iconNames) {
    if (!(Test-Path -LiteralPath (Join-Path $generatedDirectory $name))) {
        throw "Missing generated icon: $name"
    }
}
foreach ($name in $iconNames) {
    Copy-Item -LiteralPath (Join-Path $generatedDirectory $name) -Destination (Join-Path $repoRoot "src-tauri/icons/$name")
}
Copy-Item -LiteralPath $cleanLogo -Destination (Join-Path $repoRoot 'src/assets/app-logo.png')
Write-Output "Updated $changedPixels exterior pixels; interior artwork unchanged."
Write-Output "Cleared $fixedIcoCorners resampled ICO corner pixels."
Write-Output "Original and generated verification assets: $workDirectory"
