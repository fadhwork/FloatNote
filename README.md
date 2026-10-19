# FloatNote

A floating, glass-style **synced lyrics overlay** for Windows. It follows whatever you are playing
(Spotify desktop, YouTube / Spotify Web in your browser, Apple Music, …), shows the current line with a
karaoke sweep, and can turn into a **click-through, Rainmeter-style HUD** that sits on top of your games and apps.

Single portable `FloatNote.exe` – no installer, no admin rights, no login, no API keys.

## Features

- **Auto-detects what's playing** via Windows' media controls (the same source as the volume flyout).
- **Synced lyrics from [LRCLIB](https://lrclib.net)**, cached on disk so each song is fetched once. Wrong version? Search and pick another in Settings → Lyrics (remembered per song).
- **Click-through lock**: clicks pass through the lyrics. Only three things stay interactive: the **drag handle** (top-right), the **resize corner** (bottom-right) and the **timing buttons**. Toggle with **Ctrl+Alt+L** or the tray icon.
- **Fine timing**: ±0.05 s and ±0.1 s buttons (click the readout to reset). Saved **per song**.
- **Resizable, always on top**, remembers size and position (and won't restore off-screen).
- **Tray icon** keeps it out of the way: Settings, click-through, show/hide, reset position, quit. Closing Settings hides to the tray.
- **Settings** (live preview): glass opacity 0–100 % (0 = pure floating text), text size − / +, font, alignment, 1/3/5 lines, text colour, sweep colour, outline colour and width, karaoke on/off.
- No window border or outline.

## Get it

**[Download FloatNote.exe](https://github.com/fadhwork/FloatNote/releases/latest/download/FloatNote.exe)** (latest release), then just run it.
Prefer a zip? Grab `FloatNote-portable.zip` from the [Releases page](https://github.com/fadhwork/FloatNote/releases). Checksums are in `sha256.txt`.
Settings and the lyrics cache live in a `floatnote-data` folder next to the exe (delete the folder to reset).
Requires the Microsoft Edge **WebView2 runtime**, which is already part of Windows 11 and up-to-date Windows 10.

## Build it yourself

Needs [Node.js 20+](https://nodejs.org) and [Rust](https://rustup.rs) (MSVC toolchain).

```bat
build-windows.bat
```

or manually:

```bat
npm ci
npm run app:build        :: -> src-tauri\target\release\floatnote.exe  (no installer is produced)
```

**Publishing a release:** `git tag v1.1.0 && git push origin v1.1.0`. GitHub Actions builds on Windows and attaches
`FloatNote.exe`, the zip and checksums to a new Release. Every push to `main` also uploads the same files as a build artifact.

Develop with hot reload: `npm run app:dev`.
Unit tests for the Rust logic: `cd src-tauri && cargo test`.

## How it's put together

| Part | Where | What it does |
| --- | --- | --- |
| Now-playing | `src-tauri/src/media.rs` | Windows SMTC poller, extrapolates the playback position |
| Lyrics | `src-tauri/src/lyrics.rs`, `lrc.rs` | LRCLIB lookup, title cleanup, LRC parser, disk cache |
| Click-through | `src-tauri/src/lib.rs` | ~30 Hz hit-test: window ignores the mouse unless the cursor is over a control |
| Settings | `src-tauri/src/settings.rs` | Validated JSON, portable storage |
| UI | `src/overlay.ts`, `src/settings.ts` | Small vanilla TypeScript, no framework (~35 KB) |

## Notes

- The "glass" is a tinted gradient with a soft sheen. Windows can't blur the desktop behind a transparent
  webview without the janky acrylic API, so there is no real backdrop blur.
- Songs with only unsynced lyrics on LRCLIB show a hint instead of lyrics.
- A global shortcut is only registered if Ctrl+Alt+L isn't already taken by another app.

## License

Apache-2.0, see [LICENSE](LICENSE).
