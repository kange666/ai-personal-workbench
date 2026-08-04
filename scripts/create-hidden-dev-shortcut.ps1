$ErrorActionPreference = "Stop"

$projectDirectory = Split-Path -Parent $PSScriptRoot
$launcherScript = Join-Path $PSScriptRoot "start-dev-hidden.ps1"
$shortcutPath = Join-Path $projectDirectory "AI-Workbench-Dev.lnk"

if (-not (Test-Path -LiteralPath $launcherScript)) {
  throw "Hidden development launcher was not found: $launcherScript"
}

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = (Get-Command powershell.exe).Source
$shortcut.Arguments = "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$launcherScript`""
$shortcut.WorkingDirectory = $projectDirectory
$shortcut.Description = "AI Personal Workbench hidden development launcher"
$shortcut.Save()

Write-Output "Created: $shortcutPath"
