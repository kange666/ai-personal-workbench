param([string]$ChromePath = "")

$ErrorActionPreference = "Stop"
$sourceDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$prototypeDir = Split-Path -Parent $sourceDir
$wireframeDir = Join-Path $prototypeDir "wireframes"
$overviewDir = Join-Path $prototypeDir "overview"
$styleRoot = Join-Path $prototypeDir "styles"
$profileDir = Join-Path ([System.IO.Path]::GetTempPath()) ("ai-workbench-capture-" + [guid]::NewGuid().ToString("N"))

if (-not $ChromePath) {
  $browserCandidates = @(
    "C:\Users\11429\AppData\Local\Google\Chrome\Application\chrome.exe",
    "C:\Program Files\Google\Chrome\Application\chrome.exe",
    "C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
  )
  $ChromePath = $browserCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not (Test-Path -LiteralPath $ChromePath)) {
  throw "Google Chrome was not found. Pass its path through -ChromePath."
}

@($wireframeDir, $overviewDir, $styleRoot, $profileDir) | ForEach-Object {
  New-Item -ItemType Directory -Force -Path $_ | Out-Null
}

$indexUri = ([uri](Join-Path $sourceDir "index.html")).AbsoluteUri

function Capture-Page {
  param([string]$Url, [string]$OutputPath)
  $outputParent = Split-Path -Parent $OutputPath
  New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
  $browserArgs = @(
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    "--force-device-scale-factor=1",
    "--window-size=1440,900",
    "--user-data-dir=$profileDir",
    "--screenshot=$OutputPath",
    $Url
  )
  Start-Process -FilePath $ChromePath -ArgumentList $browserArgs -WindowStyle Hidden -Wait | Out-Null
  if (-not (Test-Path -LiteralPath $OutputPath)) { throw "Screenshot failed: $OutputPath" }
}

$pages = @(
  @{ Id="dashboard"; File="01-dashboard.png" },
  @{ Id="tasks"; File="02-tasks.png" },
  @{ Id="calendar"; File="03-calendar.png" },
  @{ Id="reports"; File="04-reports.png" },
  @{ Id="tokens"; File="05-tokens.png" },
  @{ Id="knowledge"; File="06-knowledge.png" }
)

foreach ($item in $pages) {
  Capture-Page "$indexUri`?page=$($item.Id)&theme=wireframe" (Join-Path $wireframeDir $item.File)
}
Capture-Page "$indexUri`?page=overview&theme=wireframe" (Join-Path $overviewDir "wireframes-overview.png")

$styles = @(
  @{ Theme="clean"; Folder="a-clean" },
  @{ Theme="command"; Folder="b-command" },
  @{ Theme="timeline"; Folder="c-timeline" }
)
$representativePages = @(
  @{ Id="dashboard"; File="01-dashboard.png" },
  @{ Id="tasks"; File="02-tasks.png" },
  @{ Id="calendar"; File="03-calendar.png" }
)

foreach ($style in $styles) {
  $targetDir = Join-Path $styleRoot $style.Folder
  foreach ($item in $representativePages) {
    Capture-Page "$indexUri`?page=$($item.Id)&theme=$($style.Theme)" (Join-Path $targetDir $item.File)
  }
  Capture-Page "$indexUri`?page=styleboard&theme=$($style.Theme)" (Join-Path $targetDir "04-styleboard.png")
}

$warmCommandDir = Join-Path (Join-Path $styleRoot "b-command") "c-theme"
foreach ($item in $representativePages) {
  Capture-Page "$indexUri`?page=$($item.Id)&theme=command&palette=timeline" (Join-Path $warmCommandDir $item.File)
}
Capture-Page "$indexUri`?page=styleboard&theme=command&palette=timeline" (Join-Path $warmCommandDir "04-styleboard.png")

Capture-Page "$indexUri`?page=comparison&theme=wireframe" (Join-Path $overviewDir "styles-comparison.png")

Get-ChildItem -LiteralPath $prototypeDir -Recurse -Filter *.png | Sort-Object FullName | Select-Object FullName, Length
