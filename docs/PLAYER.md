# Player

The player boundary accepts serializable `PlayerCommand` values and emits `PlayerState`. File paths are resolved from media IDs inside Rust and original media is never altered.

On Windows, the default development implementation opens the selected file directly with Windows Media Player. This avoids hard failing when an installed mpv build crashes on the local CPU/runtime. The mpv path remains available for explicit testing by launching the app with `CINE_WANA_USE_MPV=1`; in that mode mpv can still be controlled through the private named-pipe channel.

The quality menu lists only actual variants. Image adjustments are runtime mpv properties/filters when the mpv mode is explicitly enabled, and are persisted as profiles.
