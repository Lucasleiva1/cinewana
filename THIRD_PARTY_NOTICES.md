# Third-party notices

- Tauri: Apache-2.0 OR MIT
- React: MIT
- SQLite/rusqlite: public domain / MIT
- Montserrat (subtitle font, `mpv/fonts/Montserrat-SemiBold.ttf`): SIL Open Font License 1.1, license text shipped as `mpv/fonts/OFL-Montserrat.txt`
- FFmpeg: LGPL or GPL depending on the selected build
- mpv/libmpv: the installer ships the **LGPL-2.1-or-later** build of libmpv (`-Dgpl=false`) from mpv-winbuild, pinned in `scripts/libmpv.json` (origin, mpv commit, SHA-256 of the archive and of `libmpv-2.dll`). The installer carries `mpv/LICENSE-LGPL-2.1.txt` and `mpv/AVISO-LIBMPV.txt` next to the DLL. Never swap it for a GPL build or a `-v3` (AVX2) build without reviewing the license and the reference CPU (Intel i7-3770, no AVX2)

The setup script records download origin, version, SHA-256, and license alongside every bundled binary. Do not publish an installer until the generated manifest has been reviewed.

