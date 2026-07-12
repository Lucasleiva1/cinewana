# Updater

CINE WANA is prepared for signed Windows x64 updates through GitHub Releases.

The app checks this static manifest:

```text
https://github.com/Lucasleiva1/cinewana/releases/latest/download/latest.json
```

The embedded updater public key is stored in `apps/desktop/src-tauri/tauri.conf.json`.
The private key and password were generated outside the repository:

```text
%APPDATA%\CINE WANA\updater\tauri-updater.key
%APPDATA%\CINE WANA\updater\tauri-updater-password.txt
```

Never commit, print, or upload the private key or its password.

## Release Assets

Every Windows updater release must upload these three assets:

- `CINE.WANA_<version>_x64-setup.exe`
- `CINE.WANA_<version>_x64-setup.exe.sig`
- `latest.json`

`latest.json` must include both platform keys:

- `windows-x86_64-nsis`
- `windows-x86_64`

## Local Signed Build Later

Do not run this until you are ready to build the installer.

```powershell
$keyPath = "$env:APPDATA\CINE WANA\updater\tauri-updater.key"
$passwordPath = "$env:APPDATA\CINE WANA\updater\tauri-updater-password.txt"
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -LiteralPath $keyPath -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content -LiteralPath $passwordPath -Raw).Trim([char]0xFEFF).Trim()
corepack pnpm desktop:build
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
```

Then create the upload-ready assets:

```powershell
.\scripts\create-updater-release-assets.ps1 -Version 0.1.3 -Notes "Notas de la version"
```

Create or update the GitHub Release:

```powershell
gh release create app-v0.1.3 `
  -R Lucasleiva1/cinewana `
  target\release\bundle\release-assets-0.1.3\CINE.WANA_0.1.3_x64-setup.exe `
  target\release\bundle\release-assets-0.1.3\CINE.WANA_0.1.3_x64-setup.exe.sig `
  target\release\bundle\release-assets-0.1.3\latest.json `
  --title "CINE WANA v0.1.3" `
  --notes "Notas de la version" `
  --latest
```

If the release already exists, use `gh release upload ... --clobber`.

## Verify Manifest

After uploading, verify the endpoint:

```powershell
$endpoint = "https://github.com/Lucasleiva1/cinewana/releases/latest/download/latest.json"
$r = Invoke-WebRequest -Uri $endpoint -Headers @{ "User-Agent" = "CINE WANA" } -UseBasicParsing
$bytes = if ($r.Content -is [byte[]]) { $r.Content } else { [Text.Encoding]::UTF8.GetBytes([string]$r.Content) }
$json = ([Text.Encoding]::UTF8.GetString($bytes)) | ConvertFrom-Json
"status=$($r.StatusCode)"
"first_bytes=$(([byte[]]($bytes | Select-Object -First 3) | ForEach-Object { $_.ToString("X2") }) -join " ")"
"version=$($json.version)"
"platforms=$(([string[]]$json.platforms.PSObject.Properties.Name) -join ",")"
"signature_length=$($json.platforms."windows-x86_64-nsis".signature.Length)"
```

Expected: HTTP 200, first byte `7B`, a non-empty signature, and `windows-x86_64-nsis` present.
