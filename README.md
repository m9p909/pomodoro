# Pomodoro

A local-first desktop pomodoro and todo app for Linux, macOS, and Windows. A Rust
core owns timing, persistence, validation, and analytics; a thin Svelte 5 frontend
renders state and sends commands. Todos support optional pomodoro estimates, the
fixed 25/5/15 focus and break cycle includes a long break after every fourth focus
phase, and daily plus rolling seven-day insights show where focus time went.

The app works entirely offline. It has no accounts, cloud sync, telemetry, or
network communication, and it stores all data locally in SQLite.

## Prerequisites

- **Rust 1.90+** with `rustfmt` and `clippy`
- **Node.js 24.15+** and **pnpm 12** (the repository pins pnpm 12.8.1)
- **Tauri OS dependencies** for your platform:
  - **Linux:** `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`,
    `librsvg2-dev`, `patchelf`, plus `build-essential` and `pkg-config`
  - **macOS:** Xcode Command Line Tools
  - **Windows:** Microsoft C++ Build Tools and WebView2 (included with Windows 11
    and up-to-date Windows 10)

On Debian/Ubuntu, install the Linux packages with:

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf build-essential pkg-config
```

## Development

```bash
pnpm install
pnpm tauri dev
```

The Vite dev server runs at `http://localhost:1420` and Tauri opens the desktop
window.

## Checks and tests

```bash
pnpm check    # Svelte and TypeScript type checks
pnpm test     # frontend unit tests
```

Rust formatting, linting, and tests:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-features
```

## Building

```bash
pnpm tauri build
```

Platform-native bundles are written under `src-tauri/target/release/bundle/`.
GitHub Actions builds releases from `v*` tags: Linux x86_64 AppImage and DEB,
Windows x86_64 MSI, and a universal macOS DMG.

## TypeScript bindings

Rust command and event types generate the typed frontend bindings in
`src/lib/bindings.ts`:

```bash
pnpm bindings
```

Regenerate and commit the bindings after changing Rust commands or shared types.
CI fails if the committed bindings are out of date.

## Local data

All state lives in a SQLite database named `pomodoro.sqlite` in the OS application
data directory for the `com.pomodorocontributors.pomodoro` bundle identifier:

- Linux: `~/.local/share/com.pomodorocontributors.pomodoro/` (or under
  `$XDG_DATA_HOME`)
- macOS: `~/Library/Application Support/com.pomodorocontributors.pomodoro/`
- Windows: `%APPDATA%\com.pomodorocontributors.pomodoro\`

Back up that file to preserve your todos, focus history, and settings.

## License

MIT. See [LICENSE](LICENSE).
