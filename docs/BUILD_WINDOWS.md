# Windows build

Run `scripts/setup-windows.ps1`, then `pnpm install`, `cargo test --workspace`, and `pnpm desktop:build`. Development requires Microsoft C++ Build Tools with Desktop development with C++ and WebView2. The release target is per-user x64 NSIS.

