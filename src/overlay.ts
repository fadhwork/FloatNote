import { api, on, type Lyrics, type Media, type Rect, type Settings } from './api';
import { previewMode, getPreviewLyrics, previewMedia } from './preview';
import { applyTheme } from './theme';
import './overlay.css';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const lyricsEl = $('lyrics');
const statusEl = $('status');
const offsetEl = $('offset');
const toastEl = $('toast');
const titleEl = $('title');
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
let fillStart = 0;
let fillDur = 1;
// Karaoke sweep: one clipped copy of the line per *visual* row, so wrapped lyrics fill
// row by row (left to right, then the next row) instead of all rows at the same time.
type SweepRow = { el: HTMLElement; left: number; width: number; top: number; bottom: number };
let sweepRows: SweepRow[] = [];
let sweepTotal = 0;
let sweepW = 0;
let sweepH = 0;
let sweepCur: { wrap: HTMLElement; base: HTMLElement; text: string } | null = null;

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
  sweepRows = [];
  sweepCur = null;

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
        sweepCur = { wrap, base, text: text || '♪' };
        // The fill follows this lyric's full timestamp interval, then holds at 100%
        // until the next line becomes active. This keeps long lines slow and prevents
        // short lines from completing noticeably ahead of the audio.
        const nextStart = lines[i + 1]?.t;
        const interval = nextStart === undefined ? Math.max(2, Math.min(8, (text || '♪').length * 0.12)) : nextStart - line.t;
        fillStart = line.t;
        fillDur = Math.max(0.35, interval);
      }
      div.append(wrap);
      if (settings.romanize && line?.rom && line.rom !== line.text) {
        const romDiv = document.createElement('div');
        romDiv.className = 'rom-line';
        romDiv.textContent = line.rom;
        div.append(romDiv);
      }
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
  layoutSweep();
}

/** Measure the wrapped rows of the current line and build one sweep layer per row. */
function layoutSweep() {
  sweepRows.forEach((r) => r.el.remove());
  sweepRows = [];
  sweepTotal = 0;
  const cur = sweepCur;
  if (!cur) return;
  const { wrap, base, text } = cur;
  const wr = wrap.getBoundingClientRect();
  sweepW = wr.width;
  sweepH = wr.height;

  const range = document.createRange();
  range.selectNodeContents(base);
  const rows: { l: number; r: number; t: number; b: number }[] = [];
  for (const rc of Array.from(range.getClientRects())) {
    if (rc.width < 1 || rc.height < 1) continue;
    const mid = (rc.top + rc.bottom) / 2;
    const row = rows.find((x) => Math.abs((x.t + x.b) / 2 - mid) < rc.height * 0.5);
    if (row) {
      row.l = Math.min(row.l, rc.left);
      row.r = Math.max(row.r, rc.right);
      row.t = Math.min(row.t, rc.top);
      row.b = Math.max(row.b, rc.bottom);
    } else {
      rows.push({ l: rc.left, r: rc.right, t: rc.top, b: rc.bottom });
    }
  }
  if (rows.length === 0) rows.push({ l: wr.left, r: wr.right, t: wr.top, b: wr.bottom });
  rows.sort((a, b) => a.t - b.t);

  rows.forEach((row, i) => {
    const el = document.createElement('span');
    el.className = 'fill';
    el.textContent = text;
    el.setAttribute('aria-hidden', 'true');
    wrap.append(el);
    // Split the vertical space halfway between neighbouring rows so no glyph is cut off.
    const top = i === 0 ? 0 : (rows[i - 1].b + row.t) / 2 - wr.top;
    const bottom = i === rows.length - 1 ? wr.height : (row.b + rows[i + 1].t) / 2 - wr.top;
    const width = Math.max(1, row.r - row.l);
    sweepRows.push({ el, left: row.l - wr.left, width, top: Math.max(0, top), bottom: Math.min(wr.height, bottom) });
    sweepTotal += width;
  });
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
  if (sweepRows.length) {
    const p = Math.min(1, Math.max(0, (pos - fillStart) / fillDur));
    let dist = p * sweepTotal;
    for (const r of sweepRows) {
      const local = Math.min(1, Math.max(0, dist / r.width));
      dist -= r.width;
      if (local <= 0) {
        r.el.style.clipPath = 'inset(0 100% 0 0)';
      } else {
        const right = local >= 1 ? 0 : sweepW - r.left - local * r.width;
        r.el.style.clipPath = `inset(${r.top.toFixed(1)}px ${right.toFixed(1)}px ${(sweepH - r.bottom).toFixed(1)}px 0)`;
      }
    }
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
  if (settings.romanize && r.lines && r.lines.length > 0) {
    void fetchRomanization(loadedKey, r.lines);
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
  titleEl.textContent = [m.title, m.artist].filter(Boolean).join(' — ');
  setStatus(`${m.title}\n${m.artist}\nFinding lyrics…`);
  void fetchLyrics(m, 0);
}

function onMedia(m: Media | null) {
  if (!m) {
    loadedKey = '';
    lyrics = null;
    anchor.playing = false;
    lyricsEl.replaceChildren();
    titleEl.textContent = '';
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

if (previewMode) {
  document.body.classList.add('browser-preview');
  $('handle').title = 'Browser preview — drag to move the browser window';
  $('grip').title = 'Resize the browser window to preview layout';
} else {
  void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
    const win = getCurrentWindow();
    $('handle').addEventListener('mousedown', (e) => e.button === 0 && void win.startDragging());
    $('grip').addEventListener('mousedown', (e) => e.button === 0 && void win.startResizeDragging('SouthEast'));
  });
}
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
  const romanChanged = !first && s.romanize !== settings.romanize;
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
  if (romanChanged && lyrics && lyrics.lines.length > 0) {
    lyrics.lines.forEach((l) => (l.rom = ''));
    if (s.romanize) void fetchRomanization(loadedKey, lyrics.lines);
  }
  render();
  syncLoop();
}

async function init() {
  applySettings(await api.getSettings(), true);
  if (previewMode) {
    setStatus('BROWSER PREVIEW  ·  Midnight City — M83');
    setLyrics(getPreviewLyrics());
    loadedKey = previewMedia.key;
    onMedia({ ...previewMedia });
  } else {
    setStatus('Play a song in Spotify, YouTube Music\nor any media player.');
  }
  await on<Settings>('settings-changed', (s) => applySettings(s));
  await on<Media | null>('media', onMedia);
  await on<boolean>('hover', (inside) => document.body.classList.toggle('reveal', inside));
  await on<{ key: string; lyrics: Lyrics }>('lyrics-override', (p) => {
    loadedKey = p.key; setLyrics(p.lyrics);
  });
  let resizeTimer = 0;
  window.addEventListener('resize', () => {
    reportRegions();
    // Rows re-wrap when the window is resized: re-measure the sweep once the drag settles.
    clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      layoutSweep();
      render();
    }, 100);
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

/** Latin-letter readings for kanji / kana / hangul / ... lines, so you can sing along. */
async function fetchRomanization(curKey: string, lines: Lyrics['lines']) {
  try {
    const out = await api.romanizeLyrics(curKey, lines.map((l) => l.text));
    if (curKey !== loadedKey || !lyrics || lyrics.lines !== lines || !settings.romanize) return;
    lines.forEach((l, i) => {
      l.rom = out[i]?.trim() || '';
    });
    shownSig = '';
    render();
  } catch (err) {
    // Romanization is optional: keep showing the original lyrics, but say why nothing appeared.
    if (curKey === loadedKey && settings.romanize) toast(`Romanization failed: ${String(err).slice(0, 140)}`);
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
