# Deja libmpv-2.dll adentro del proyecto (apps/desktop/src-tauri/lib), que es de donde la toma el
# instalador de CINE WANA. El archivo pesa ~97 MB y GitHub no acepta archivos de más de 100 MB en el
# historial, por eso no se guarda en git: este script lo baja exacto y verificado cuando falta.
param([switch]$Force)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'libmpv.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$libDir = Join-Path $projectRoot 'apps\desktop\src-tauri\lib'
$dll = Join-Path $libDir 'libmpv-2.dll'

if (-not $Force -and (Test-Path -LiteralPath $dll)) {
  $current = (Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash.ToLower()
  if ($current -eq $manifest.dllSha256) {
    Write-Host "libmpv ya está en el proyecto y es la versión correcta."
    return
  }
  Write-Host "La libmpv del proyecto no es la versión fijada; se vuelve a bajar."
}

$sevenZip = @('C:\Program Files\7-Zip\7z.exe', 'C:\Program Files (x86)\7-Zip\7z.exe') | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $sevenZip) { throw 'Hace falta 7-Zip para descomprimir libmpv (https://www.7-zip.org).' }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("cinewana-libmpv-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
  $archive = Join-Path $work 'libmpv.7z'
  Write-Host "Bajando $($manifest.archiveUrl)"
  Invoke-WebRequest -UseBasicParsing -Uri $manifest.archiveUrl -OutFile $archive
  $archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLower()
  if ($archiveHash -ne $manifest.archiveSha256) { throw "El archivo bajado no coincide con la huella esperada ($archiveHash)." }
  & $sevenZip x -y "-o$work\extraido" $archive 'libmpv-2.dll' | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "7-Zip no pudo descomprimir libmpv (código $LASTEXITCODE)." }
  $extracted = Join-Path $work 'extraido\libmpv-2.dll'
  $dllHash = (Get-FileHash -LiteralPath $extracted -Algorithm SHA256).Hash.ToLower()
  if ($dllHash -ne $manifest.dllSha256) { throw "libmpv-2.dll no coincide con la huella esperada ($dllHash)." }
  New-Item -ItemType Directory -Force -Path $libDir | Out-Null
  Copy-Item -LiteralPath $extracted -Destination $dll -Force
  Write-Host "libmpv lista en $dll"
} finally {
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
