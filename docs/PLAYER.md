# Player

The player boundary accepts serializable `PlayerCommand` values and emits `PlayerState`. The Windows implementation loads libmpv dynamically and uses an embedded native surface. File paths are resolved from media IDs inside Rust. Original media is never altered.

The quality menu lists only actual variants. Image adjustments are runtime mpv properties/filters and are persisted as profiles.

