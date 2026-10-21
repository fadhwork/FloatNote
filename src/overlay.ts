import { getCurrentWindow } from '@tauri-apps/api/window';
import { api, on, type Lyrics, type Media, type Rect, type Settings } from './api';
import { applyTheme } from './theme';
import './overlay.css';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const lyricsEl = $('lyrics');
const statusEl = $('status');
const offsetEl = $('offset');
const toastEl = $('toast');
setStatus('FloatNote starting…');

let settings: Settings;
let lyrics: Lyrics | null = null;
let currentMedia: Media | null = null;
let loadedKey = '';
let offsetMs = 0;
let offsetTimer = 0;
let anchor = { pos: 0, at: performance.now(), playing: false };
let lastActiveAt = performance.now(); // for fade-when-idle
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
  if (!lyrics || !lyrics.lines || lyrics.lines.length === 0) return;
  const lines = lyrics.lines;
  const lineCount = Number(settings?.lines) || 3;
  const half = Math.floor((lineCount - 1) / 2);
  lyricsEl.classList.toggle('plain', !settings.karaoke);
  lyricsEl.replaceChildren();
  fillEl = null;

  // Intro preview before the first line starts
  if (idx < 0) {
    const div = document.createElement('div');
    div.className = 'line cur';
    const wrap = document.createElement('span');
    wrap.className = 'wrap';
    const base = document.createElement('span');
    base.className = 'base';
    base.textContent = lines[0] ? `♪  ${lines[0].text || '...'}` : '♪';
    wrap.append(base);
    div.append(wrap);
    lyricsEl.append(div);

    if (half > 0 && lines[1]) {
      const nextDiv = document.createElement('div');
      nextDiv.className = 'line near n1';
      nextDiv.textContent = lines[1].text || '';
      lyricsEl.append(nextDiv);
    }
    return;
  }

  for (let i = idx - half; i <= idx + half; i++) {
    const dist = Math.abs(i - idx);
    const line = lines[i];
    const text = line ? line.text || '♪' : '';
    const div = document.createElement('div');
    div.className = dist === 0 ? 'line cur' : `line near n${dist}`;

    if (dist === 0) {
      const wrap = document.createElement('span');
      wrap.className = 'wrap';
      const base = document.createElement('span');
      base.className = 'base';
      base.textContent = text || '♪';
      wrap.append(base);
      if (settings.karaoke && line && !lyrics.approx) {
        const fill = document.createElement('span');
        fill.className = 'fill';
        fill.textContent = text || '♪';
        fill.setAttribute('aria-hidden', 'true');
        wrap.append(fill);
        fillEl = fill;
        const end = lines[i + 1]?.t ?? line.t + 4;
        const gap = Math.max(0.3, end - line.t);
        fillStart = line.t;
        fillDur = Math.max(0.3, Math.min(gap * 0.9, Math.max(1.2, (text || '♪').length * 0.085)));
      }
      div.append(wrap);
      if (settings.dualSubtitles && line?.sub && line.sub !== line.text) {
        const subDiv = document.createElement('div');
        subDiv.className = 'sub-line';
        subDiv.textContent = line.sub;
        div.append(subDiv);
      }
    } else {
      div.textContent = text;
    }
    lyricsEl.append(div);
  }
  lyricsEl.classList.toggle('one', settings.lines === 1);
  if (settings.lines === 1) fitOneLine();
}

/** One-line mode: shrink the current line (never below 50%) so it fits the width instead of wrapping. */
function fitOneLine() {
  lyricsEl.style.setProperty('--fit', '1');
  const wrap = lyricsEl.querySelector<HTMLElement>('.line.cur .wrap');
  if (!wrap) return;
  const avail = lyricsEl.clientWidth - 48;
  const need = wrap.scrollWidth;
  if (avail > 0 && need > avail) lyricsEl.style.setProperty('--fit', String(Math.max(0.5, avail / need)));
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
  $('playBtn').classList.toggle('playing', anchor.playing);
  updateIdle();
  const wantLoop = anchor.playing && !!lyrics && lyrics.lines.length > 0;
  if (wantLoop && !raf) raf = requestAnimationFrame(frame);
  if (!wantLoop) {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    render();
  }
}

/** One line that shows what FloatNote currently sees, so a failure is never silent. */
function diag(): string {
  if (!currentMedia) return '';
  const pos = Math.max(0, position());
  const mm = Math.floor(pos / 60);
  const ss = String(Math.floor(pos % 60)).padStart(2, '0');
  const app = currentMedia.source.replace(/\.exe$/i, '') || 'player';
  const clock = currentMedia.clock === 'own' ? ' · own clock' : '';
  return `${app} · ${anchor.playing ? 'playing' : 'paused'} · ${mm}:${ss}${clock}`;
}

function setLyrics(r: Lyrics) {
  if (r.approx && r.lines.length > 0 && currentMedia && currentMedia.durationMs > 0 && r.duration > 0) {
    // Lines were spread over LRCLIB's song length; stretch them to the length that is really playing.
    const k = currentMedia.durationMs / 1000 / r.duration;
    if (Math.abs(k - 1) > 0.01) {
      r.lines.forEach((l) => (l.t *= k));
      r.duration = currentMedia.durationMs / 1000;
    }
  }
  lyrics = r;
  shownSig = '';
  lyricsEl.replaceChildren();
  if (r.instrumental) {
    setStatus('♪  Instrumental');
  } else if (!r.lines || r.lines.length === 0) {
    const who = currentMedia ? `${currentMedia.title} — ${currentMedia.artist}` : 'this song';
    setStatus(`Couldn't find synced lyrics automatically for\n${who}\n${diag()}\nYou can pick a version in Settings → Lyrics.`);
  } else {
    setStatus('');
    render();
    if (r.approx) toast('No synced lyrics for this song. Timing is estimated: use −1s / +1s to line it up.');
  }
  syncLoop();
  if (settings.dualSubtitles && r.lines && r.lines.length > 0) {
    void fetchTranslation(loadedKey, r.lines);
  }
}

async function fetchLyrics(m: Media, attempt: number) {
  try {
    const r = await api.getLyrics(m);
    if (m.key === loadedKey) setLyrics(r);
  } catch (err) {
    if (m.key !== loadedKey) return;
    const reason = String(err).slice(0, 140);
    if (attempt < 3) {
      setStatus(`Lyrics lookup failed: ${reason}\nRetrying…\n${diag()}`);
      setTimeout(() => m.key === loadedKey && fetchLyrics(m, attempt + 1), 8000);
    } else {
      setStatus(`Lyrics lookup failed: ${reason}\nCheck your internet connection.\n${diag()}`);
    }
  }
}

function startTrack(m: Media) {
  currentMedia = m;
  loadedKey = m.key;
  lyrics = null;
  shownSig = '';
  offsetMs = (settings?.offsets && settings.offsets[m.key]) ?? 0;
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
  if (trackChanged || m.playing !== anchor.playing || Math.abs(incoming - expected) > 1.2) {
    anchor = { pos: incoming, at: now, playing: m.playing };
  }
  if (trackChanged) { startTrack(m); if (settings.dynamicColor) void fetchAlbumArtColor(m); }
  syncLoop();
}

// ---- timing ------------------------------------------------------------------

function showOffset() {
  const s = offsetMs / 1000;
  offsetEl.textContent = offsetMs === 0 ? '0' : `${s > 0 ? '+' : ''}${+s.toFixed(2)}s`;
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
  else if (btn.dataset.d !== undefined) adjust(Number(btn.dataset.d));
});

// ---- window controls -----------------------------------------------------------

const win = getCurrentWindow();
$('handle').addEventListener('mousedown', (e) => e.button === 0 && void win.startDragging());
$('grip').addEventListener('mousedown', (e) => e.button === 0 && void win.startResizeDragging('SouthEast'));
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
  const translateChanged = !first && (s.dualSubtitles !== settings.dualSubtitles || s.translateTo !== settings.translateTo);
  settings = s;
  applyTheme(s);
  document.body.classList.toggle('locked', s.clickThrough);
  shownSig = '';
  if (loadedKey && !offsetTimer) {
    offsetMs = (s?.offsets && s.offsets[loadedKey]) ?? 0;
    showOffset();
  }
  if (lockChanged) {
    toast(s.clickThrough ? 'Click-through on  ·  Ctrl+Alt+L or the tray icon to unlock' : 'Click-through off');
  }
  requestAnimationFrame(reportRegions);
  if (translateChanged && lyrics && lyrics.lines.length > 0) {
    lyrics.lines.forEach((l) => (l.sub = ''));
    if (s.dualSubtitles) void fetchTranslation(loadedKey, lyrics.lines);
  }
  render();
  syncLoop();
}

async function init() {
  applySettings(await api.getSettings(), true);
  setStatus('Play a song in Spotify, YouTube Music\nor any media player.');
  await on<Settings>('settings-changed', (s) => applySettings(s));
  await on<Media | null>('media', onMedia);
  await on<boolean>('hover', (inside) => document.body.classList.toggle('reveal', inside));
  await on<{ key: string; lyrics: Lyrics }>('lyrics-override', (p) => {
    loadedKey = p.key; setLyrics(p.lyrics);
  });
  window.addEventListener('resize', () => {
    reportRegions();
    if (settings.lines === 1) {
      shownSig = '';
      render();
    }
  });
}

init().catch((err) => setStatus(`Startup error: ${String(err).slice(0, 160)}`));

// ---- media controls & drag-and-drop .lrc -------------------------------------
$('prevBtn')?.addEventListener('click', () => void api.mediaControl('prev'));
$('playBtn')?.addEventListener('click', () => void api.mediaControl('toggle'));
$('nextBtn')?.addEventListener('click', () => void api.mediaControl('next'));

document.addEventListener('dragover', (e) => {
  e.preventDefault();
  document.body.classList.add('drag-over');
});
document.addEventListener('dragleave', () => {
  document.body.classList.remove('drag-over');
});
document.addEventListener('drop', async (e) => {
  e.preventDefault();
  document.body.classList.remove('drag-over');
  const file = e.dataTransfer?.files[0];
  if (!file || !file.name.endsWith('.lrc')) return;

  const text = await file.text();
  const parsedLines: { t: number; text: string }[] = [];
  const regex = /\[(\d{2}):(\d{2})\.(\d{2,3})\](.*)/;

  text.split('\n').forEach((line) => {
    const match = regex.exec(line);
    if (match) {
      const m = parseInt(match[1], 10);
      const s = parseInt(match[2], 10);
      const ms = parseInt(match[3].padEnd(3, '0').slice(0, 3), 10);
      parsedLines.push({ t: m * 60 + s + ms / 1000, text: match[4].trim() });
    }
  });

  if (parsedLines.length > 0 && loadedKey) {
    const customLyrics: Lyrics = {
      lines: parsedLines,
      instrumental: false,
      source: 'manual',
      title: file.name.replace('.lrc', ''),
      artist: 'Local file',
      duration: parsedLines[parsedLines.length - 1].t,
    };
    await api.applyLyrics(loadedKey, customLyrics);
    toast('Custom .lrc applied!');
  }
});

// ---- Dynamic Accent Color & Dual Subtitles ---------------------------------
function extractVibrantColor(url: string): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      try {
        const canvas = document.createElement('canvas');
        canvas.width = 40;
        canvas.height = 40;
        const ctx = canvas.getContext('2d');
        if (!ctx) return resolve(null);
        ctx.drawImage(img, 0, 0, 40, 40);
        const data = ctx.getImageData(0, 0, 40, 40).data;
        let bestColor: string | null = null;
        let bestScore = -1;
        for (let i = 0; i < data.length; i += 16) {
          const r = data[i], g = data[i + 1], b = data[i + 2];
          const max = Math.max(r, g, b), min = Math.min(r, g, b);
          const d = max - min;
          const s = max === 0 ? 0 : d / max;
          const l = (max + min) / 510;
          if (l < 0.25 || l > 0.85) continue;
          const score = s * (1 - Math.abs(l - 0.55));
          if (score > bestScore) {
            bestScore = score;
            bestColor = `#${((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1)}`;
          }
        }
        resolve(bestColor);
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

async function fetchAlbumArtColor(m: Media) {
  try {
    const artUrl = await api.getAlbumArt(m.title, m.artist);
    if (!artUrl || m.key !== loadedKey) return;
    const color = await extractVibrantColor(artUrl);
    if (color && m.key === loadedKey) {
      document.documentElement.style.setProperty('--fill-color', color);
    }
  } catch {
    // Fallback to default fillColor
  }
}

async function fetchTranslation(curKey: string, lines: Lyrics['lines']) {
  try {
    const out = await api.translateLyrics(curKey, lines.map((l) => l.text), settings.translateTo);
    if (curKey !== loadedKey || !lyrics || lyrics.lines !== lines) return;
    lines.forEach((l, i) => {
      l.sub = out[i]?.trim() || '';
    });
    shownSig = '';
    render();
  } catch {
    // Translation is optional: keep showing the original lyrics.
  }
}

window.addEventListener('error', (e) => {
  console.error('FloatNote overlay error:', e.error || e.message);
  if (!lyrics || lyrics.lines.length === 0) setStatus(`Error: ${String(e.message).slice(0, 160)}`);
});
window.addEventListener('unhandledrejection', (e) => {
  console.error('FloatNote unhandled promise:', e.reason);
  if (!lyrics || lyrics.lines.length === 0) setStatus(`Error: ${String(e.reason).slice(0, 160)}`);
});


// ---- fade when idle: lyrics and glass fade out after a few seconds without playback ----
function updateIdle() {
  const active = !!loadedKey && anchor.playing;
  if (active) lastActiveAt = performance.now();
  const idle = !!settings?.idleFade && !active && performance.now() - lastActiveAt > 8000;
  document.body.classList.toggle('idle', idle);
}
setInterval(updateIdle, 1000);
