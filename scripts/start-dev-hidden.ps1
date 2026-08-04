$ErrorActionPreference = "Stop"
$projectDirectory = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectDirectory

$visualStudioCommand = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path -LiteralPath $visualStudioCommand)) { exit 1 }

$environmentLines = & cmd.exe /d /s /c "`"$visualStudioCommand`" -arch=x64 -host_arch=x64 >nul && set"
foreach ($line in $environmentLines) {
  if ($line -match "^([^=]+)=(.*)$") { Set-Item -Path "Env:$($matches[1])" -Value $matches[2] }
}
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

if (-not (Test-Path -LiteralPath (Join-Path $projectDirectory "node_modules"))) {
  & npm.cmd install
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

& npm.cmd run dev:desktop
exit $LASTEXITCODE
