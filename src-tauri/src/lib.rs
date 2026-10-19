mod lrc;
mod lyrics;
mod media;
mod settings;

use lyrics::LyricsResult;
use serde::Deserialize;
use serde_json::{json, Value};
use settings::{Geometry, Settings};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WindowEvent, Wry,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// An interactive region of the overlay, in CSS pixels relative to the window.
#[derive(Deserialize, Clone, Debug)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(&self, px: f64, py: f64) -> bool {
        const PAD: f64 = 4.0;
        px >= self.x - PAD && px <= self.x + self.w + PAD && py >= self.y - PAD && py <= self.y + self.h + PAD
    }
}

struct AppState {
    settings: Mutex<Settings>,
    hit: Mutex<Vec<Rect>>,
    data_dir: PathBuf,
    lock_item: Mutex<Option<CheckMenuItem<Wry>>>,
    http: reqwest::Client,
}

// ---- helpers ----------------------------------------------------------------

/// Portable: keep data next to the exe. Fall back to the user profile if that folder is read-only.
fn resolve_data_dir(app: &AppHandle) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("floatnote-data");
            if std::fs::create_dir_all(&p).is_ok() {
                let probe = p.join(".write-test");
                if std::fs::write(&probe, b"x").is_ok() {
                    let _ = std::fs::remove_file(&probe);
                    return p;
                }
            }
        }
    }
    let fallback = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("floatnote"));
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

fn apply_patch(app: &AppHandle, patch: Value) -> Result<Settings, String> {
    let state = app.state::<AppState>();
    let new = {
        let mut s = state.settings.lock().unwrap();
        s.merge(patch)?;
        s.clone()
    };
    settings::save(&state.data_dir, &new);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_always_on_top(new.always_on_top);
    }
    if let Some(item) = state.lock_item.lock().unwrap().as_ref() {
        let _ = item.set_checked(new.click_through);
    }
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

fn toggle_lock(app: &AppHandle) {
    let current = {
        let state = app.state::<AppState>();
        let v = state.settings.lock().unwrap().click_through;
        v
    };
    let _ = apply_patch(app, json!({ "clickThrough": !current }));
}

fn open_settings_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn reset_position(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_size(PhysicalSize::new(900u32, 320u32));
        let _ = w.center();
        let _ = w.show();
    }
    let _ = apply_patch(app, json!({ "clickThrough": false, "window": null }));
}

// ---- commands ---------------------------------------------------------------

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn update_settings(app: AppHandle, patch: Value) -> Result<Settings, String> {
    apply_patch(&app, patch)
}

#[tauri::command]
fn set_hit_regions(state: State<'_, AppState>, rects: Vec<Rect>) {
    *state.hit.lock().unwrap() = rects;
}

#[tauri::command]
fn set_offset(app: AppHandle, key: String, ms: i64) -> Result<(), String> {
    let mut offsets = {
        let state = app.state::<AppState>();
        let o = state.settings.lock().unwrap().offsets.clone();
        o
    };
    if ms == 0 {
        offsets.remove(&key);
    } else {
        offsets.insert(key, ms.clamp(-30_000, 30_000));
    }
    apply_patch(&app, json!({ "offsets": offsets })).map(|_| ())
}

#[tauri::command]
async fn get_lyrics(
    state: State<'_, AppState>,
    key: String,
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
) -> Result<LyricsResult, String> {
    let dir = state.data_dir.clone();
    let client = state.http.clone();
    if let Some(hit) = lyrics::load_cached(&dir, &key) {
        return Ok(hit);
    }
    match lyrics::fetch(&client, &title, &artist, &album, duration_ms as f64 / 1000.0).await? {
        Some(found) => {
            lyrics::store_cached(&dir, &key, &found);
            Ok(found)
        }
        None => Ok(LyricsResult {
            source: "none".into(),
            ..Default::default()
        }),
    }
}

#[tauri::command]
async fn search_lyrics(state: State<'_, AppState>, query: String) -> Result<Vec<LyricsResult>, String> {
    let client = state.http.clone();
    lyrics::search(&client, query.trim()).await
}

/// The user picked a different lyric version: remember it for this song and tell the overlay.
#[tauri::command]
fn apply_lyrics(app: AppHandle, key: String, lyrics: LyricsResult) {
    let mut lyrics = lyrics;
    lyrics.source = "manual".into();
    let dir = app.state::<AppState>().data_dir.clone();
    lyrics::store_cached(&dir, &key, &lyrics);
    let _ = app.emit("lyrics-override", json!({ "key": key, "lyrics": lyrics }));
}

#[tauri::command]
fn hide_settings(app: AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.hide();
        let done = app.state::<AppState>().settings.lock().unwrap().first_run_done;
        if !done {
            let _ = apply_patch(&app, json!({ "firstRunDone": true }));
        }
    }
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    open_settings_window(&app);
}

#[tauri::command]
fn media_control(action: String) {
    media::control(&action);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---- background loop: click-through hit testing + window geometry ------------------

/// Rainmeter-style click-through. While locked, the whole window ignores the mouse
/// except while the cursor is over a registered control (drag handle, resize grip,
/// timing buttons). Also remembers window size/position.
fn spawn_window_loop(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_ignore: Option<bool> = None;
        let mut last_inside = false;
        let mut tick: u32 = 0;
        let mut last_geom: Option<Geometry> = None;
        let mut dirty_since: Option<Instant> = None;

        loop {
            std::thread::sleep(Duration::from_millis(33));
            tick = tick.wrapping_add(1);
            let Some(win) = app.get_webview_window("main") else { continue };
            if !win.is_visible().unwrap_or(false) {
                continue;
            }
            let state = app.state::<AppState>();
            let click_through = state.settings.lock().unwrap().click_through;

            let inside = match (win.cursor_position(), win.inner_position(), win.scale_factor()) {
                (Ok(c), Ok(p), Ok(sf)) if sf > 0.0 => {
                    let x = (c.x - p.x as f64) / sf;
                    let y = (c.y - p.y as f64) / sf;
                    state.hit.lock().unwrap().iter().any(|r| r.contains(x, y))
                }
                _ => true,
            };

            let want_ignore = click_through && !inside;
            if last_ignore != Some(want_ignore) {
                let _ = win.set_ignore_cursor_events(want_ignore);
                last_ignore = Some(want_ignore);
            }
            if inside != last_inside {
                last_inside = inside;
                let _ = app.emit("hover", inside);
            }

            // Persist geometry ~700 ms after the user stops moving/resizing.
            if tick % 10 == 0 {
                if let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) {
                    let g = Geometry { x: p.x, y: p.y, w: s.width, h: s.height };
                    let changed = match &last_geom {
                        Some(l) => l.x != g.x || l.y != g.y || l.w != g.w || l.h != g.h,
                        None => false,
                    };
                    if changed {
                        dirty_since = Some(Instant::now());
                    }
                    last_geom = Some(g);
                }
            }
            if let Some(t) = dirty_since {
                if t.elapsed() > Duration::from_millis(700) {
                    dirty_since = None;
                    if let Some(g) = last_geom.clone() {
                        let _ = apply_patch(&app, json!({ "window": g }));
                    }
                }
            }
        }
    });
}

// ---- app entry --------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_lock(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            set_hit_regions,
            set_offset,
            get_lyrics,
            search_lyrics,
            apply_lyrics,
            open_settings,
            hide_settings,
            media_control,
            quit_app
        ])
        .on_window_event(|window, event| {
            if window.label() != "settings" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Closing settings just tucks FloatNote away in the tray.
                api.prevent_close();
                let _ = window.hide();
                let app = window.app_handle();
                let done = app.state::<AppState>().settings.lock().unwrap().first_run_done;
                if !done {
                    let _ = apply_patch(app, json!({ "firstRunDone": true }));
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = resolve_data_dir(&handle);
            let loaded = settings::load(&data_dir);

            let http = reqwest::Client::builder()
                .user_agent(lyrics::UA)
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|e| e.to_string())?;

            // Tray ("hidden icon on the corner toolbar")
            let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let lock_i = CheckMenuItem::with_id(
                app,
                "lock",
                "Click-through   (Ctrl+Alt+L)",
                true,
                loaded.click_through,
                None::<&str>,
            )?;
            let vis_i = MenuItem::with_id(app, "toggle", "Show / hide lyrics", true, None::<&str>)?;
            let reset_i = MenuItem::with_id(app, "reset", "Reset position", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit FloatNote", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &lock_i, &vis_i, &reset_i, &sep, &quit_i])?;

            let mut tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("FloatNote")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "settings" => open_settings_window(app),
                    "lock" => toggle_lock(app),
                    "toggle" => {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                            }
                        }
                    }
                    "reset" => reset_position(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        open_settings_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            let first_run = !loaded.first_run_done;
            let saved_window = loaded.window.clone();
            let on_top = loaded.always_on_top;

            app.manage(AppState {
                settings: Mutex::new(loaded),
                hit: Mutex::new(Vec::new()),
                data_dir,
                lock_item: Mutex::new(Some(lock_i)),
                http,
            });

            // Restore the overlay's size/position (only if it is still on a monitor).
            if let Some(win) = app.get_webview_window("main") {
                if let Some(g) = saved_window {
                    let on_screen = win
                        .available_monitors()
                        .map(|ms| {
                            ms.iter().any(|m| {
                                let p = m.position();
                                let s = m.size();
                                g.x >= p.x - 50
                                    && g.x < p.x + s.width as i32 - 50
                                    && g.y >= p.y - 50
                                    && g.y < p.y + s.height as i32 - 50
                            })
                        })
                        .unwrap_or(false);
                    if on_screen {
                        let _ = win.set_size(PhysicalSize::new(g.w, g.h));
                        let _ = win.set_position(PhysicalPosition::new(g.x, g.y));
                    }
                }
                let _ = win.set_always_on_top(on_top);
                let _ = win.show();
            }

            // Ctrl+Alt+L toggles click-through (you cannot click the overlay while it is locked).
            let _ = app
                .global_shortcut()
                .register(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL));

            if first_run {
                open_settings_window(&handle);
            }

            media::spawn(handle.clone());
            spawn_window_loop(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FloatNote");
}
