# STS2 Mod Translation Manager

Slay the Spire 2 mod translation and management tool.

This project provides a Windows desktop app and CLI for managing Slay the Spire 2 mods, extracting language resources, and preparing translation workflows.

## Features

- Detect installed Slay the Spire 2 mods.
- Enable, disable, and organize mods with presets.
- Extract mod language resources for translation.
- Preview and compare translation data.
- Manage translation workspaces and merge outputs.
- Launch and check game/mod setup status.
- Use a Korean-first Tauri desktop UI with English/Korean language toggle.

## Requirements

- Windows
- Rust toolchain
- Node.js and npm

## Development

Install dependencies:

```powershell
npm install
```

Run the Tauri desktop app in development mode:

```powershell
npm run tauri dev
```

Build the web UI:

```powershell
npm run build
```

Build the Rust release binary:

```powershell
.\scripts\build-release.ps1
```

## CLI

After building, the CLI binary is available at:

```text
target\release\sts2_mod_manager.exe
```

Common commands:

```powershell
.\target\release\sts2_mod_manager.exe scan
.\target\release\sts2_mod_manager.exe ui
.\target\release\sts2_mod_manager.exe preset list
.\target\release\sts2_mod_manager.exe translation list
.\target\release\sts2_mod_manager.exe launch status
.\target\release\sts2_mod_manager.exe tools status
```

Running the executable without arguments prints CLI help.

## License

MIT
