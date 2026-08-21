param(
    [string]$PrivateKeyPath = "$env:USERPROFILE\.tauri\ai-personal-workbench.key",
    [string]$PrivateKeyPassword = "",
    [string]$ReleaseNotes = "Feature updates and stability improvements.",
    [switch]$AllowDirty
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$package = Get-Content -LiteralPath (Join-Path $projectRoot "package.json") -Raw -Encoding UTF8 | ConvertFrom-Json
$tauriConfig = Get-Content -LiteralPath (Join-Path $projectRoot "src-tauri\tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
$version = [string]$package.version

if ($version -ne [string]$tauriConfig.version) {
    throw "Version mismatch between package.json and tauri.conf.json."
}
if (-not (Test-Path -LiteralPath $PrivateKeyPath)) {
    throw "Updater private key is missing: $PrivateKeyPath. Generate it with the Tauri signer command first."
}
$protectedPasswordPath = "$PrivateKeyPath.password.dpapi"
if ([string]::IsNullOrEmpty($PrivateKeyPassword) -and (Test-Path -LiteralPath $protectedPasswordPath)) {
    $protectedPassword = (Get-Content -LiteralPath $protectedPasswordPath -Raw).Trim()
    $securePassword = $protectedPassword | ConvertTo-SecureString
    $passwordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($securePassword)
    try {
        $PrivateKeyPassword = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPointer)
    }
    finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($passwordPointer)
    }
}
if ([string]::IsNullOrEmpty($PrivateKeyPassword)) {
    throw "Updater key password is unavailable. Pass -PrivateKeyPassword or restore the DPAPI password file."
}
$dirtyFiles = git -C $projectRoot status --porcelain
if ($dirtyFiles -and -not $AllowDirty) {
    throw "The worktree is dirty. Commit the confirmed release first, or pass -AllowDirty for local validation only."
}

$previousKey = $env:TAURI_SIGNING_PRIVATE_KEY
$previousPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
$env:TAURI_SIGNING_PRIVATE_KEY = $PrivateKeyPath
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $PrivateKeyPassword

Push-Location $projectRoot
try {
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "Tauri release build failed." }
}
finally {
    Pop-Location
    $env:TAURI_SIGNING_PRIVATE_KEY = $previousKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $previousPassword
}

$releaseExe = Join-Path $projectRoot "src-tauri\target\release\ai-personal-workbench.exe"
$nsisDirectory = Join-Path $projectRoot "src-tauri\target\release\bundle\nsis"
$installer = Get-ChildItem -LiteralPath $nsisDirectory -Filter "*.exe" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $installer) { throw "NSIS installer was not found." }

$signaturePath = "$($installer.FullName).sig"
if (-not (Test-Path -LiteralPath $signaturePath)) { throw "Updater signature was not found: $signaturePath" }
if (-not (Test-Path -LiteralPath $releaseExe)) { throw "Portable executable was not found: $releaseExe" }

$tag = "V$version"
$outputDirectory = Join-Path $projectRoot "release\$tag"
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$installerName = "AI-Personal-Workbench-$tag-Installer.exe"
$portableName = "AI-Personal-Workbench-$tag-Portable.exe"
$installerOutput = Join-Path $outputDirectory $installerName
$portableOutput = Join-Path $outputDirectory $portableName
$signatureOutput = "$installerOutput.sig"
Copy-Item -LiteralPath $installer.FullName -Destination $installerOutput -Force
Copy-Item -LiteralPath $releaseExe -Destination $portableOutput -Force
Copy-Item -LiteralPath $signaturePath -Destination $signatureOutput -Force

# Preserve the exact Pages directory layout required by the in-app updater.
$pagesMirrorDirectory = Join-Path $outputDirectory "pages-mirror\downloads\$tag"
New-Item -ItemType Directory -Path $pagesMirrorDirectory -Force | Out-Null
Copy-Item -LiteralPath $installerOutput -Destination (Join-Path $pagesMirrorDirectory $installerName) -Force

$installerItem = Get-Item -LiteralPath $installerOutput
$portableItem = Get-Item -LiteralPath $portableOutput
$installerHash = (Get-FileHash -LiteralPath $installerOutput -Algorithm SHA256).Hash
$portableHash = (Get-FileHash -LiteralPath $portableOutput -Algorithm SHA256).Hash
$signature = (Get-Content -LiteralPath $signatureOutput -Raw).Trim()
$releaseBaseUrl = "https://github.com/kange666/ai-personal-workbench-download/releases/download/$tag"
$updaterMirrorUrl = "https://kange666.github.io/ai-personal-workbench-download/downloads/$tag/$installerName"
$sourceCommit = (git -C $projectRoot rev-parse HEAD).Trim()
$publishedAt = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")

$manifest = [ordered]@{
    version = $version
    notes = $ReleaseNotes
    pub_date = $publishedAt
    publishedAt = $publishedAt
    releaseUrl = "https://github.com/kange666/ai-personal-workbench-download/releases/tag/$tag"
    sourceUrl = "https://github.com/kange666/ai-personal-workbench"
    sourceCommit = $sourceCommit
    installer = [ordered]@{
        name = $installerName
        sizeBytes = $installerItem.Length
        sizeText = "{0:N2} MB" -f ($installerItem.Length / 1MB)
        sha256 = $installerHash
        url = "$releaseBaseUrl/$installerName"
    }
    portable = [ordered]@{
        name = $portableName
        sizeBytes = $portableItem.Length
        sizeText = "{0:N2} MB" -f ($portableItem.Length / 1MB)
        sha256 = $portableHash
        url = "$releaseBaseUrl/$portableName"
    }
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $signature
            # The updater uses the Pages mirror to avoid GitHub Release redirect stalls.
            url = $updaterMirrorUrl
        }
    }
}

$manifestPath = Join-Path $outputDirectory "release.json"
$manifestJson = $manifest | ConvertTo-Json -Depth 6
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($manifestPath, $manifestJson, $utf8WithoutBom)

Write-Host "Signed release generated: $outputDirectory"
Write-Host "Publish the installer, portable executable, and release.json for $tag."
