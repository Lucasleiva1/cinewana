param(
  [Parameter(Mandatory = $true)]
  [string]$Version,

  [string]$OwnerRepo = "Lucasleiva1/cinewana",
  [string]$Tag = "app-v$Version",
  [string]$Notes = "Actualizacion CINE WANA $Version"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$nsisDir = Join-Path $repoRoot "target\release\bundle\nsis"
$assetDir = Join-Path $repoRoot "artifacts\releases\v$Version"

$sourceExe = Get-ChildItem -LiteralPath $nsisDir -Filter "*$Version*_x64-setup.exe" |
  Select-Object -First 1

if (-not $sourceExe) {
  throw "No se encontro el instalador NSIS para la version $Version en $nsisDir. Primero ejecuta el build firmado."
}

$sourceSig = "$($sourceExe.FullName).sig"
if (-not (Test-Path -LiteralPath $sourceSig)) {
  throw "No se encontro la firma $sourceSig. El build debe generar .exe y .exe.sig."
}

New-Item -ItemType Directory -Force -Path $assetDir | Out-Null

$assetExeName = "CINE.WANA_${Version}_x64-setup.exe"
$assetExe = Join-Path $assetDir $assetExeName
$assetSig = Join-Path $assetDir "$assetExeName.sig"

Copy-Item -LiteralPath $sourceExe.FullName -Destination $assetExe -Force
Copy-Item -LiteralPath $sourceSig -Destination $assetSig -Force

$signature = (Get-Content -LiteralPath $assetSig -Raw).Trim()
$url = "https://github.com/$OwnerRepo/releases/download/$Tag/$assetExeName"
$manifest = [ordered]@{
  version = $Version
  notes = $Notes
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64-nsis" = [ordered]@{ signature = $signature; url = $url }
    "windows-x86_64" = [ordered]@{ signature = $signature; url = $url }
  }
}

$latestPath = Join-Path $assetDir "latest.json"
$json = $manifest | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText([System.IO.Path]::GetFullPath($latestPath), $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "Release assets listos en $assetDir"
Write-Host "- $assetExeName"
Write-Host "- $assetExeName.sig"
Write-Host "- latest.json"
