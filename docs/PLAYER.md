# Player

The player boundary has two paths. The primary path is the internal CINE WANA WebView player: the frontend resolves the selected media ID through Rust, receives the local file path, plays it with an in-app `<video>` surface, and saves watch progress through `save_progress`. Original media is never altered.

The secondary path is external playback. Users can explicitly choose "Abrir externo", which delegates to the Rust `PlayerCommand` service. On Windows this opens the selected file directly with Windows Media Player by default to avoid hard failing when an installed mpv build crashes on the local CPU/runtime. The mpv path remains available for explicit testing by launching the app with `CINE_WANA_USE_MPV=1`; in that mode mpv can still be controlled through the private named-pipe channel.

The internal player includes playback controls, seeking, volume, fullscreen, progress saving, image scanning, and non-destructive runtime image adjustments. Fine-grained audio/subtitle selectors remain pending.
