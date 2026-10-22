# FloatNote

[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078d4.svg?style=flat-square)](https://github.com/fadhwork/FloatNote)
[![Release](https://img.shields.io/github/v/release/fadhwork/FloatNote?color=38bdf8&style=flat-square)](https://github.com/fadhwork/FloatNote/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=flat-square)](LICENSE)
[![Author](https://img.shields.io/badge/author-fadhlihsani-lightgrey.svg?style=flat-square)](https://github.com/fadhwork)

A modern, transparent, click-through **synced lyrics HUD** for Windows. It detects whatever music you are playing, renders synchronized lyrics with a karaoke sweep, and floats unobtrusively over your apps and games.

Single portable `FloatNote.exe` — zero install, zero login, no telemetry.

---

## ✨ Features

- **Universal Media Sync:** Automatically connects to Spotify, Apple Music, YouTube (Chrome, Edge, Firefox), Tidal, and VLC via Windows SMTC.
- **Accurate Synced Lyrics:** Powered by [LRCLIB](https://lrclib.net) with local disk caching for instant playback.
- **Dual Subtitles & Translation:** Automatic real-time English translation for non-English songs (J-Pop, K-Pop, anime OSTs, Spanish, etc.).
- **Dynamic Accent Colors:** Samples the vibrant color palette from the current track's album art.
- **Click-Through HUD:** Toggle with **Ctrl+Alt+L** to make the overlay click-through so it never interrupts gameplay or work.
- **On-Hover Media Controls:** Quick **⏮ ⏯ ⏭** buttons and text size toggles appear right on the overlay.
- **Smart Auto-Dim:** Smoothly fades down when music is paused, restoring instantly on playback.
- **Drag & Drop `.lrc`:** Drag any custom or local `.lrc` file directly onto the HUD for immediate override.
- **Extensive Styling:** Custom fonts, opacity, font size, stroke outlines, alignment, and line count (1, 3, or 5 lines), with independent translation and sing-along text colors and sizes.

---

## 🚀 Download & Setup

1. Grab **[`FloatNote.exe`](https://github.com/fadhwork/FloatNote/releases/latest)** from the latest release.
2. Place it in any folder and run it.
3. Start playing music in your preferred player or browser tab.

> **Note:** FloatNote is fully portable. Settings and lyrics cache are stored in a small `floatnote-data` folder alongside the executable.

---

## ⌨️ Shortcuts & Controls

| Shortcut / Action | Function |
| :--- | :--- |
| **Ctrl + Alt + L** | Toggle Click-Through Lock |
| **Hover Overlay** | Reveal media controls, text sizing, and window controls |
| **Drag Grip (Top-Right)** | Reposition the HUD |
| **Resize Grip (Bottom-Right)** | Resize window dimensions |
| **System Tray Icon** | Access Settings, toggle lyrics visibility, or reset position |

---

## 🛠️ Building from Source

**Prerequisites:** [Node.js 20+](https://nodejs.org) and [Rust](https://rustup.rs) (MSVC toolchain).

```bat
# One-click Windows build
build-windows.bat
```

Or manually:

```bash
npm ci
npm run app:build
```

The compiled binary will be located in `src-tauri/target/release/floatnote.exe`.

---

## 👤 Author

Created and maintained by **fadhlihsani** ([@fadhwork](https://github.com/fadhwork)).

## 📄 License

Distributed under the **Apache-2.0** License. See [`LICENSE`](LICENSE) for details.
