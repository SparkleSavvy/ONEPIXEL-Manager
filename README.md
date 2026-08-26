# ONEPIXEL Manager

A lightweight desktop build manager for the [ONEPIXEL Minecraft modpack](https://github.com/SparkleSavvy/ONEPIXEL). Built with Tauri 2 and Svelte 5, dark monochrome UI.

## Features

- **Versions** — browse every published release of the modpack straight from GitHub.
  - Download any `.mrpack` client pack
  - Download full `.zip` archives (for launchers without mrpack support)
  - Download ready-to-run **server packs** where available
  - Streaming downloads with progress bars, cancel support and SHA-256 integrity checks against GitHub digests
- **Library** — everything downloaded on this machine.
  - **Install to launcher**: hands the mrpack to your launcher of choice — Prism-compatible launchers are invoked with the `-I <file>` CLI flag which opens their native import dialog; XMCL / custom launchers receive the file path as an argument, falling back to the system file association when no executable is configured
  - Launcher selection with automatic detection: **ElyPrism**, **Prism Launcher**, **XMCL**, or any custom executable
  - **Server management**: extract a server pack, start/stop it as a managed process with a live log console
    - Automatic acceptance of the ServerPackCreator "Type 'I agree'" prompt, so automated Java (Jabba) installation proceeds unattended
    - Send commands to the running server from the built-in console (`stop`, `say hello`, …)
    - Toggle `online-mode` in `server.properties` with one switch (applies on next server start)
  - Delete versions and server packs (with guards while downloads are active or servers are running)
- **Settings**
  - Launcher kind + executable path (with native file picker)
  - Self-update check — inert for now, activates automatically once the manager's own repository is published

## Getting started

### Prerequisites

- Node.js 20+ and npm
- Rust (stable) with the MSVC toolchain on Windows
- WebView2 runtime (preinstalled on Windows 10/11)

### Run in development

```sh
npm install
npm run tauri dev
```

### Production build

```sh
npm run tauri build
```

Installers/bundles are written to `src-tauri/target/release/bundle/`.

### Checks

```sh
npm run check        # svelte-check + TypeScript
npm run build        # frontend production bundle
cargo clippy         # Rust linting (run inside src-tauri/)
```

## Data layout

All data lives under `%LOCALAPPDATA%\onepixel-manager\`:

```
onepixel-manager/
├── versions/<tag>/      # downloaded mrpack / zip files
├── servers/<tag>/       # extracted server packs (+ world data once generated)
└── config.json          # launcher choice and settings
```

Deleting a version in the Library removes its folder permanently.

## Notes

- Changing `online-mode` requires a server restart to take effect.
- Self-update: the Settings page compares releases of
  [SparkleSavvy/ONEPIXEL-Manager](https://github.com/SparkleSavvy/ONEPIXEL-Manager)
  against the running version. Override with `managerRepo` (`owner/name`) in
  `config.json`, or set it to an invalid value to disable checks.
- Releases that contain no modpack assets (e.g. tool-only releases) are filtered out automatically.

## Related

- Modpack repository: [SparkleSavvy/ONEPIXEL](https://github.com/SparkleSavvy/ONEPIXEL)

## License

GPL-3.0 — same as the ONEPIXEL modpack.
