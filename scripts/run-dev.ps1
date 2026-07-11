$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$ffmpeg = Get-ChildItem -Path "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Gyan.FFmpeg.Shared_*\ffmpeg-*\bin" -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
if ($ffmpeg) { $env:Path = "$($ffmpeg.FullName);$env:Path" }
if (Test-Path -LiteralPath 'C:\Program Files\MPV Player') { $env:Path = "C:\Program Files\MPV Player;$env:Path" }
Set-Location -LiteralPath $root
corepack.cmd pnpm desktop:dev

