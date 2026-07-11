$ErrorActionPreference = 'Stop'

function Require-WingetPackage([string]$Id) {
  $installed = winget list --id $Id --exact --accept-source-agreements 2>$null
  if ($LASTEXITCODE -ne 0) {
    winget install --id $Id --exact --silent --accept-package-agreements --accept-source-agreements
  }
}

Require-WingetPackage 'Microsoft.VisualStudio.2022.BuildTools'
Require-WingetPackage 'Gyan.FFmpeg.Shared'
Require-WingetPackage 'shinchiro.mpv'
corepack.cmd prepare pnpm@10.13.1 --activate
corepack.cmd pnpm install
Write-Host 'CINE WANA development dependencies are ready.' -ForegroundColor Green

