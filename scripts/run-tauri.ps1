param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('dev', 'build')]
  [string]$Mode
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$envFile = Join-Path $projectRoot '.env'

if (Test-Path -LiteralPath $envFile) {
  foreach ($line in Get-Content -LiteralPath $envFile -Encoding UTF8) {
    if ($line -match '^\s*(TMDB_READ_ACCESS_TOKEN|TMDB_API_KEY)\s*=\s*(.*?)\s*$') {
      [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
    }
  }
}

Set-Location -LiteralPath $projectRoot
corepack.cmd pnpm --filter @cine-wana/desktop tauri $Mode
