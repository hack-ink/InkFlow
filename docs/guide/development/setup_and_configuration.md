# Setup and Configuration

This document provides example configuration fragments for the first working implementation.

## `src-tauri/tauri.conf.json` (Essential Fields)

Notes:

- `app.macOSPrivateApi = true` enables macOS private APIs required for transparent windows.
- `transparent = true` is required for window effects.
- `windowEffects` applies the glass material; adjust for your preferred look.

```json
{
  "$schema": "./gen/schemas/config.schema.json",
  "productName": "InkFlow",
  "version": "0.1.0",
  "identifier": "ink.hack.inkflow",
  "build": {
    "frontendDist": "../ui/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "deno task --cwd ../ui dev",
    "beforeBuildCommand": "deno task --cwd ../ui build",
    "removeUnusedCommands": true
  },
  "app": {
    "withGlobalTauri": true,
    "macOSPrivateApi": true,
    "windows": [
      {
        "label": "main",
        "url": "index.html",
        "visible": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "resizable": false,
        "width": 720,
        "height": 64,
        "title": "InkFlow",
        "shadow": true,
        "windowEffects": {
          "effects": ["hudWindow"],
          "state": "active",
          "radius": 14
        }
      }
    ]
  }
}
```

## `src-tauri/capabilities/default.json` (Minimal Example)

This must be tailored after commands and plugins are added. Use the generated schema for autocompletion.

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for the main (overlay) window.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "allow-overlay-set-height",
    "allow-session-dispatch",
    "allow-settings-get",
    "allow-settings-update",
    "allow-platform-open-system-settings"
  ]
}
```

## `src-tauri/permissions/*.toml` (App Command Permissions)

App-level `tauri::command` entries must be declared as permissions before they can be referenced by capabilities.

Example (`src-tauri/permissions/app.toml`):

```toml
[[permission]]
identifier = "allow-session-dispatch"
description = "Enables the session_dispatch command."
commands.allow = ["session_dispatch"]
```

## `src-tauri/Info.plist` (Microphone)

```xml
<key>NSMicrophoneUsageDescription</key>
<string>InkFlow needs microphone access to transcribe your speech into text.</string>
```

## `src-tauri/Cargo.toml` (Feature Gates)

Recommended structure:

```toml
[features]
default = ["platform-macos"]
platform-macos = []
platform-windows = []
platform-linux = []
```

## `ui/deno.json` (Deno Tasks + nodeModulesDir)

This is the preferred approach for running UI tooling with Deno.

```json
{
  "nodeModulesDir": "auto",
  "tasks": {
    "dev": "vite --port 1420 --strictPort",
    "build": "vite build",
    "preview": "vite preview --port 1420 --strictPort",
    "fmt": "deno fmt",
    "lint": "deno lint"
  }
}
```

## UI Dependencies (Recommended)

This is the baseline dependency set for “liquid glass” + smooth animations:

- `react`, `react-dom`
- `@tauri-apps/api`
- `vite`, `@vitejs/plugin-react`
- `tailwindcss`, `postcss`, `autoprefixer`
- `framer-motion`
- `@radix-ui/react-dialog`, `@radix-ui/react-popover` (and other primitives as needed)
- `clsx` (class composition)
- `lucide-react` (icons)

Install using Deno local installation (example):

```sh
cd ui
deno install --dev npm:vite npm:@vitejs/plugin-react
deno install npm:react npm:react-dom
deno install npm:@tauri-apps/api
deno install npm:tailwindcss npm:postcss npm:autoprefixer
deno install npm:framer-motion
deno install npm:@radix-ui/react-dialog npm:@radix-ui/react-popover
deno install npm:clsx npm:lucide-react
```

## Cargo Make (Repository Rule)

Use these tasks for Rust formatting and validation:

- `cargo make fmt`
- `cargo make clippy`
- `cargo make nextest`
