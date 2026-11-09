# Player

The player boundary has two paths. The primary path is the internal CINE WANA WebView player: the frontend resolves the selected media ID through Rust, receives the local file path, plays it with an in-app `<video>` surface, and saves watch progress through `save_progress`. Original media is never altered.

The secondary path is external playback. Users can explicitly choose "Abrir externo", which delegates to the Rust `PlayerCommand` service. On Windows this opens the selected file directly with Windows Media Player by default to avoid hard failing when an installed mpv build crashes on the local CPU/runtime. The mpv path remains available for explicit testing by launching the app with `CINE_WANA_USE_MPV=1`; in that mode mpv can still be controlled through the private named-pipe channel.

The internal player includes playback controls, seeking, volume, fullscreen, progress saving, image scanning, and non-destructive runtime image adjustments. Fine-grained audio/subtitle selectors remain pending.

## libmpv player (branch `reproductor-mpv`)

The internal player is moving to libmpv, which renders the video inside the CINE WANA window under the
React interface:

- **Library**: `libmpv-2.dll` (LGPL, x86_64 without AVX2) ships inside the installer under `mpv/`.
  `scripts/setup-libmpv.ps1` downloads the pinned build from `scripts/libmpv.json` and verifies both
  SHA-256 values. The app never looks for an mpv installed on the computer.
- **Safety probe**: the app runs itself with `--cinewana-probar-libmpv <dll>` before loading the DLL. If
  that probe fails or times out, `App.tsx` mounts the previous WebView player instead.
- **Embedding**: libmpv receives the main window handle as `wid`; the window is created transparent and
  the page becomes transparent only while the mpv player shell is in the DOM (CSS `html:has(.cw-player-shell.mpv)`, so there is no gap on open or close). mpv's child
  window is pushed to the bottom of the z-order on every video reconfiguration.
- **Commands and state**: `embedded_player_open`, `embedded_player_command` (typed `EmbeddedCommand`),
  `embedded_player_stop`, `embedded_player_status`. State changes arrive as the
  `embedded-player-state` event; positions are throttled to 200 ms, everything else is immediate.
- **Tracks**: audio and subtitles come from mpv's `track-list`; the user's choice is stored per title in
  `%APPDATA%/com.cinewana.app/mpv/pistas-elegidas.json` and restored when the file loads. Spanish
  (latino first) is preferred for audio and subtitles; subtitles that match the audio language are only
  auto-selected when forced.
- **Image**: contrast, saturation, gamma and hue use mpv's own controls; brightness, shadows,
  highlights, temperature and sharpness use `cinewana-imagen.glsl` (GPU, live `glsl-shader-opts`). The rest
  are mpv options: scaling (`scale`/`cscale`/`dscale`), `scale-antiring`, `deband` + `deband-iterations`,
  `deinterlace`, `interpolation` with `video-sync=display-resample`, `panscan`, `video-aspect-override`,
  `video-zoom`, `video-rotate`, and for HDR titles `tone-mapping` and `hdr-compute-peak`. Only the
  properties that changed are sent. Nothing heavy is on by default. The FFmpeg scene scan is not offered
  in this player.
- **Subtitles and controls**: while the controls are visible `sub-pos` is raised above them, and it goes
  back to the bottom when they hide.
- **Diagnostics**: the last session log is written to `%APPDATA%/com.cinewana.app/mpv/mpv-ultimo.log`.

