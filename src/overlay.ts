import { getCurrentWindow } from '@tauri-apps/api/window';
import { api, on, type Lyrics, type Media, type Rect, type Settings } from './api';
import { applyTheme } from './theme';
import './overlay.css';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const lyricsEl = $('lyrics');
const statusEl = $('status');
const offsetEl = $('offset');
const toastEl = $('toast');

let settings: Settings;
let lyrics: Lyrics | null = null;
let loadedKey = '';
let offsetMs = 0;
let offsetTimer = 0;
let anchor = { pos: 0, at: performance.now(), playing: false };
let raf = 0;

// What is currently drawn, so we only rebuild the DOM when the line changes.
let shownSig = '';
let fillEl: HTMLElement | null = null;
let fillStart = 0;
let fillDur = 1;

const position = () =>
  anchor.pos + (anchor.playing ? (performance.now() - anchor.at) / 1000 : 0) + offsetMs / 1000;

/** Index of the last line whose start time is <= pos, or -1 before the first line. */
function findIndex(lines: Lyrics['lines'], pos: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].t <= pos) {
      ans = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return ans;
}

function setStatus(text: string) {
  statusEl.textContent = text;
  statusEl.style.display = text ? 'grid' : 'none';
}

function build(idx: number) {
  if (!lyrics) return;
  const lines = lyrics.lines;
  const half = (settings.lines - 1) / 2;
  lyricsEl.classList.toggle('plain', !settings.karaoke);
  lyricsEl.replaceChildren();
  fillEl = null;

  for (let i = idx - half; i <= idx + half; i++) {
    const dist = Math.abs(i - idx);
    const line = lines[i];
    const text = line ? line.text || '♪' : i === idx ? '♪' : '';
    const div = document.createElement('div');
    div.className = dist === 0 ? 'line cur' : `line near n${dist}`;

    if (dist === 0) {
      const wrap = document.createElement('span');
      wrap.className = 'wrap';
      const base = document.createElement('span');
      base.className = 'base';
      base.textContent = text;
      wrap.append(base);
      if (settings.karaoke && line) {
        const fill = document.createElement('span');
        fill.className = 'fill';
        fill.textContent = text;
        fill.setAttribute('aria-hidden', 'true');
        wrap.append(fill);
        fillEl = fill;
        const end = lines[i + 1]?.t ?? line.t + 4;
        const gap = Math.max(0.3, end - line.t);
        fillStart = line.t;
        fillDur = Math.max(0.3, Math.min(gap * 0.9, Math.max(1.2, text.length * 0.085)));
      }
      div.append(wrap);
    } else {
      div.textContent = text;
    }
    lyricsEl.append(div);
  }
}

function render() {
  if (!lyrics || lyrics.lines.length === 0) return;
  const pos = position();
  const idx = findIndex(lyrics.lines, pos);
  const sig = `${loadedKey}|${idx}|${settings.lines}|${settings.karaoke}`;
  if (sig !== shownSig) {
    shownSig = sig;
    build(idx);
  }
  if (fillEl) {
    const p = Math.min(1, Math.max(0, (pos - fillStart) / fillDur));
    fillEl.style.clipPath = `inset(0 ${((1 - p) * 100).toFixed(2)}% 0 0)`;
  }
}

function frame() {
  render();
  raf = requestAnimationFrame(frame);
}

/** Animate only while a song is actually playing; otherwise draw once and stop. */
function syncLoop() {
  const wantLoop = anchor.playing && !!lyrics && lyrics.lines.length > 0;
  if (wantLoop && !raf) raf = requestAnimationFrame(frame);
  if (!wantLoop) {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    render();
  }
}

function setLyrics(r: Lyrics) {
  lyrics = r;
  shownSig = '';
  lyricsEl.replaceChildren();
  if (r.instrumental) setStatus('♪  Instrumental');
  else if (r.lines.length === 0) setStatus('No synced lyrics found for this song.\nOpen Settings → Lyrics to search manually.');
  else setStatus('');
  syncLoop();
}

async function fetchLyrics(m: Media, attempt: number) {
  try {
    const r = await api.getLyrics(m);
    if (m.key === loadedKey) setLyrics(r);
  } catch {
    if (m.key !== loadedKey) return;
    if (attempt < 3) {
      setStatus("Couldn't reach the lyrics service.\nRetrying…");
      setTimeout(() => m.key === loadedKey && fetchLyrics(m, attempt + 1), 8000);
    } else {
      setStatus("Couldn't reach the lyrics service.\nCheck your internet connection.");
    }
  }
}

function startTrack(m: Media) {
  loadedKey = m.key;
  lyrics = null;
  shownSig = '';
  offsetMs = settings.offsets[m.key] ?? 0;
  showOffset();
  lyricsEl.replaceChildren();
  setStatus(`${m.title}\n${m.artist}\nFinding lyrics…`);
  void fetchLyrics(m, 0);
}

function onMedia(m: Media | null) {
  if (!m) {
    loadedKey = '';
    lyrics = null;
    anchor.playing = false;
    lyricsEl.replaceChildren();
    setStatus('Play a song in Spotify, YouTube Music\nor any media player.');
    syncLoop();
    return;
  }
  const now = performance.now();
  const incoming = m.positionMs / 1000;
  const expected = anchor.pos + (anchor.playing ? (now - anchor.at) / 1000 : 0);
  const trackChanged = m.key !== loadedKey;
  // Re-anchor only on real changes (seek, pause/play, new song) so the sweep never jitters.
  if (trackChanged || m.playing !== anchor.playing || Math.abs(incoming - expected) > 0.3) {
    anchor = { pos: incoming, at: now, playing: m.playing };
  }
  if (trackChanged) startTrack(m);
  syncLoop();
}

// ---- timing ------------------------------------------------------------------

function showOffset() {
  const s = offsetMs / 1000;
  offsetEl.textContent = `${s > 0 ? '+' : ''}${s.toFixed(2)}s`;
}

function adjust(deltaMs: number) {
  offsetMs = Math.max(-30000, Math.min(30000, deltaMs === 0 ? 0 : offsetMs + deltaMs));
  showOffset();
  render();
  if (!loadedKey) return;
  clearTimeout(offsetTimer);
  const key = loadedKey;
  const value = offsetMs;
  offsetTimer = window.setTimeout(() => {
    offsetTimer = 0;
    void api.setOffset(key, value);
  }, 300);
}

$('timing').addEventListener('click', (e) => {
  const btn = (e.target as HTMLElement).closest('button');
  if (!btn) return;
  if (btn.id === 'offset') adjust(0);
  else adjust(Number(btn.dataset.d));
});

// ---- window controls -----------------------------------------------------------

const win = getCurrentWindow();
$('handle').addEventListener('mousedown', (e) => e.button === 0 && void win.startDragging());
$('grip').addEventListener('mousedown', (e) => e.button === 0 && void win.startResizeDragging('SouthEast'));
$('fontDown').addEventListener('click', () => void api.update({ fontPx: settings.fontPx - 2 }));
$('fontUp').addEventListener('click', () => void api.update({ fontPx: settings.fontPx + 2 }));
$('lock').addEventListener('click', () => void api.update({ clickThrough: true }));
$('gear').addEventListener('click', () => void api.openSettings());
document.addEventListener('contextmenu', (e) => e.preventDefault());

/** Tell Rust which parts of the overlay must stay clickable while click-through is on. */
function reportRegions() {
  const rects: Rect[] = [];
  document.querySelectorAll<HTMLElement>('[data-hit]').forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.width > 0 && r.height > 0) rects.push({ x: r.x, y: r.y, w: r.width, h: r.height });
  });
  void api.setHitRegions(rects);
}

let toastTimer = 0;
function toast(text: string) {
  toastEl.textContent = text;
  toastEl.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toastEl.classList.remove('show'), 3800);
}

function applySettings(s: Settings, first = false) {
  const lockChanged = !first && s.clickThrough !== settings.clickThrough;
  settings = s;
  applyTheme(s);
  document.body.classList.toggle('locked', s.clickThrough);
  shownSig = '';
  if (loadedKey && !offsetTimer) {
    offsetMs = s.offsets[loadedKey] ?? 0;
    showOffset();
  }
  if (lockChanged) {
    toast(s.clickThrough ? 'Click-through on  ·  Ctrl+Alt+L or the tray icon to unlock' : 'Click-through off');
  }
  requestAnimationFrame(reportRegions);
  syncLoop();
}

async function init() {
  applySettings(await api.getSettings(), true);
  setStatus('Play a song in Spotify, YouTube Music\nor any media player.');
  await on<Settings>('settings-changed', (s) => applySettings(s));
  await on<Media | null>('media', onMedia);
  await on<boolean>('hover', (inside) => document.body.classList.toggle('reveal', inside));
  await on<{ key: string; lyrics: Lyrics }>('lyrics-override', (p) => {
    if (p.key === loadedKey) setLyrics(p.lyrics);
  });
  window.addEventListener('resize', reportRegions);
}

void init();
