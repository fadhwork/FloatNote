import { api, on, type Lyrics, type Media, type Settings } from './api';
import { BUILT_IN, currentLook, sameLook, type Preset } from './presets';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';
import './settings.css';

const app = document.getElementById('app')!;
app.innerHTML = `
<header class="top">
  <div class="brand">FloatNote</div>
  <div class="np">
    <div class="marq" id="nowTitleBox"><b id="nowTitle">Nothing playing</b></div>
    <span id="nowArtist">Start a song in Spotify, YouTube Music or any player</span>
  </div>
</header>

<section class="presets">
  <h2>Presets</h2>
  <div class="bar">
    <span id="presetChips" class="chips"></span>
    <input id="presetName" type="text" maxlength="24" placeholder="Name this look to keep it" />
    <button class="primary" id="savePreset">Save look</button>
  </div>
</section>

<div class="cols">
  <section>
    <h2>Text and order</h2>
    <div class="row"><label for="fontFamily">Font</label>
      <div class="ctl slider-col"><select id="fontFamily"><option value="sans">Modern Sans</option><option value="display">Display</option><option value="serif">Editorial Serif</option><option value="mono">Monospace</option></select></div></div>
    <div class="row"><span class="lbl">Align</span><div class="ctl slider-col"><div class="seg icons" id="align">
      <button data-v="left" title="Align left" aria-label="Align left"><svg viewBox="0 0 16 16"><path d="M2 3h12M2 6.5h8M2 10h12M2 13.5h8"/></svg></button>
      <button data-v="center" title="Align center" aria-label="Align center"><svg viewBox="0 0 16 16"><path d="M2 3h12M4 6.5h8M2 10h12M4 13.5h8"/></svg></button>
      <button data-v="right" title="Align right" aria-label="Align right"><svg viewBox="0 0 16 16"><path d="M2 3h12M6 6.5h8M2 10h12M6 13.5h8"/></svg></button>
    </div></div></div>
    <div class="row"><span class="lbl">Lines</span><div class="ctl slider-col"><div class="seg" id="lines"><button data-v="1">1</button><button data-v="3">3</button><button data-v="5">5</button></div></div></div>
    <div class="row slider-color"><label for="opacity">Glass Opacity</label>
      <div class="ctl color-slider">
        <input id="opacity" type="range" min="0" max="100" />
        <input id="opacityNum" class="num" type="number" min="0" max="100" step="1" aria-label="Glass opacity" />
        <label class="color-pick" title="Glass colour"><input id="glassColor" type="color" aria-label="Glass colour" /></label>
      </div>
    </div>
    <div class="row slider-color"><label for="fontPx">Lyric</label>
      <div class="ctl color-slider"><input id="fontPx" type="range" min="14" max="72" /><input id="fontPxNum" class="num" type="number" min="12" max="120" step="1" aria-label="Text size in px" /><label class="color-pick" title="Text colour"><input id="fontColor" type="color" aria-label="Text colour" /></label></div></div>
    <div class="row slider-color"><label for="outlineWidth">Outline</label>
      <div class="ctl color-slider"><input id="outlineWidth" type="range" min="0" max="6" step="0.5" /><input id="outlineWidthNum" class="num" type="number" min="0" max="6" step="0.5" aria-label="Outline size" /><label class="color-pick" title="Outline colour"><input id="outlineColor" type="color" aria-label="Outline colour" /></label></div></div>
    <div class="row slider-color"><label for="subFontPx">Translation</label>
      <div class="ctl color-slider"><input id="subFontPx" type="range" min="8" max="80" /><input id="subFontPxNum" class="num" type="number" min="8" max="80" step="1" aria-label="Translation text size in px" /><label class="color-pick" title="Translation text colour"><input id="subColor" type="color" aria-label="Translation text colour" /></label></div></div>
    <div class="row slider-color"><label for="romFontPx">Romanize</label>
      <div class="ctl color-slider"><input id="romFontPx" type="range" min="8" max="80" /><input id="romFontPxNum" class="num" type="number" min="8" max="80" step="1" aria-label="Romanization text size in px" /><label class="color-pick" title="Sing-along text colour"><input id="romColor" type="color" aria-label="Romanization text colour" /></label></div></div>
    <div class="row slider-color"><label for="menuOpacity">Menu Glass</label>
      <div class="ctl color-slider">
        <input id="menuOpacity" type="range" min="0" max="100" />
        <input id="menuOpacityNum" class="num" type="number" min="0" max="100" step="1" aria-label="Menu glass opacity" />
        <label class="color-pick" title="Menu glass colour"><input id="menuColor" type="color" aria-label="Menu glass colour" /></label>
      </div>
    </div>
    <div class="row"><span class="lbl">Title &amp; icons</span><div class="ctl"><label class="color-pick" title="Song / artist title colour"><input id="titleColor" type="color" aria-label="Title colour" /></label><label class="color-pick" title="Menu icon colour"><input id="iconColor" type="color" aria-label="Icon colour" /></label></div></div>
    <div class="row"><label for="karaoke">Karaoke</label><div class="ctl"><label class="switch"><input id="karaoke" type="checkbox" /><span></span></label><label class="color-pick" title="Sweep colour"><input id="fillColor" type="color" aria-label="Sweep colour" /></label></div></div>
  </section>

  <section>
    <h2>Behaviour</h2>
    <div class="row"><label for="clickThrough">Click-through (lock)</label><div class="ctl"><label class="switch"><input id="clickThrough" type="checkbox" /><span></span></label></div></div>
    <div class="row"><label for="onTop">Always on top</label><div class="ctl"><label class="switch"><input id="onTop" type="checkbox" /><span></span></label></div></div>
    <div class="row"><label for="gameMode" title="When a game or a fullscreen video is in front">Fullscreen apps</label>
      <div class="ctl"><select id="gameMode"><option value="clickthrough">Click-through</option><option value="hide">Hide lyrics</option><option value="off">Do nothing</option></select></div></div>
    <div class="row"><label for="autostart">Start with Windows</label><div class="ctl"><label class="switch"><input id="autostart" type="checkbox" /><span></span></label></div></div>
    <div class="row"><label for="idleFade" title="Hides after 5 s paused or 3 s stopped; comes back when the music plays">Hide when paused</label><div class="ctl"><label class="switch"><input id="idleFade" type="checkbox" /><span></span></label></div></div>
    <div class="row"><label for="dynamicColor">Album accent</label><div class="ctl"><label class="switch"><input id="dynamicColor" type="checkbox" /><span></span></label></div></div>
    <div class="row"><label for="romanize">Romanize (sing along)</label><div class="ctl"><label class="switch"><input id="romanize" type="checkbox" /><span></span></label></div></div>
    <div class="row"><label for="translate">Translate lyrics</label>
      <div class="ctl"><select id="translate"><option value="off">Off</option><option value="en">English</option><option value="id">Indonesia</option><option value="es">Español</option><option value="fr">Français</option><option value="de">Deutsch</option><option value="pt">Português</option><option value="ja">日本語</option><option value="ko">한국어</option><option value="zh-CN">中文</option></select></div></div>
  </section>
</div>

<section>
  <h2>Lyrics</h2>
  <div class="row"><span class="lbl">Timing, this song</span><div class="ctl"><span class="val" id="songOffset">0s</span><button id="resetOffset">Reset</button></div></div>
  <div class="row"><input id="query" type="text" placeholder="Wrong lyrics? Search title, artist" /><button class="primary" id="search">Search</button></div>
  <div id="results"></div>
</section>

<div class="actions">
  <span class="version" id="version"></span>
  <button id="hide">Hide to tray</button>
  <button class="danger" id="quit">Quit</button>
</div>

<footer>
  <span class="credit">crafted with love by fadhlihsani</span>
</footer>
`;

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let s: Settings = {
  opacity: 35,
  glassColor: '#24283a',
  fontPx: 30,
  fontColor: '#ffffff',
  fillColor: '#7dd3fc',
  subColor: '#b8e7ff',
  titleColor: '#ffffff',
  iconColor: '#ffffff',
  menuColor: '#24283a',
  menuOpacity: 55,
  subFontPx: 14,
  romColor: '#ffffff',
  romFontPx: 16,
  outline: true,
  outlineColor: '#000000',
  outlineWidth: 2,
  fontFamily: 'sans',
  align: 'center',
  lines: 3,
  karaoke: true,
  clickThrough: false,
  alwaysOnTop: true,
  firstRunDone: true,
  dynamicColor: false,
  dualSubtitles: false,
  romanize: false,
  translateTo: 'en',
  startWithWindows: false,
  idleFade: true,
  offsets: {},
  gameMode: 'clickthrough',
  presets: [],
};

let media: Media | null = null;
let pending: Partial<Settings> = {};
let frame = 0;

function patch(p: Partial<Settings>) {
  Object.assign(pending, p);
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    const out = pending;
    pending = {};
    void api.update(out);
  });
}

/** Write a value into an input unless the user is currently typing / dragging in it. */
function setIfIdle(id: string, value: string) {
  const input = el<HTMLInputElement>(id);
  if (document.activeElement !== input) input.value = value;
}

/** Built-in templates first, then the user's own looks (those can be deleted). */
function renderPresets() {
  const box = el('presetChips');
  box.replaceChildren();
  const add = (p: Preset, mine: boolean) => {
    const chip = document.createElement('div');
    chip.className = 'chip' + (sameLook(s, p.values) ? ' on' : '');
    const apply = document.createElement('button');
    apply.className = 'chipbtn';
    apply.title = mine ? 'Use your saved look' : 'Use this template';
    const dot = document.createElement('span');
    dot.className = 'dot';
    dot.style.background = `linear-gradient(135deg, ${String(p.values.glassColor)} 50%, ${String(p.values.fillColor)} 50%)`;
    apply.append(dot, p.name);
    apply.addEventListener('click', () => patch(p.values as Partial<Settings>));
    chip.append(apply);
    if (mine) {
      const del = document.createElement('button');
      del.className = 'chipx';
      del.textContent = '×';
      del.title = 'Delete this look';
      del.addEventListener('click', () => patch({ presets: s.presets.filter((q) => q.name !== p.name) }));
      chip.append(del);
    }
    box.append(chip);
  };
  BUILT_IN.forEach((p) => add(p, false));
  s.presets.forEach((p) => add(p, true));
}

function saveLook() {
  const input = el<HTMLInputElement>('presetName');
  const name = input.value.trim().slice(0, 24);
  if (!name) {
    input.focus();
    return;
  }
  const others = s.presets.filter((p) => p.name.toLowerCase() !== name.toLowerCase());
  if (others.length >= 12) {
    input.value = '';
    input.placeholder = 'Limit is 12: delete one first';
    return;
  }
  patch({ presets: [...others, { name, values: currentLook(s) }] });
  input.value = '';
  input.placeholder = 'Name this look to keep it';
}
el('savePreset').addEventListener('click', saveLook);
el('presetName').addEventListener('keydown', (e) => {
  if ((e as KeyboardEvent).key === 'Enter') saveLook();
});

let presetSig = '';
function sync() {
  if (!s) return;
  const sig = JSON.stringify([currentLook(s), s.presets]);
  if (sig !== presetSig) {
    presetSig = sig;
    renderPresets();
  }
  setIfIdle('opacity', String(s.opacity));
  setIfIdle('opacityNum', String(s.opacity));
  setIfIdle('fontPx', String(s.fontPx));
  setIfIdle('fontPxNum', String(s.fontPx));
  setIfIdle('subFontPx', String(s.subFontPx));
  setIfIdle('subFontPxNum', String(s.subFontPx));
  setIfIdle('romFontPx', String(s.romFontPx));
  setIfIdle('romFontPxNum', String(s.romFontPx));
  el<HTMLSelectElement>('fontFamily').value = s.fontFamily;
  el<HTMLSelectElement>('gameMode').value = s.gameMode;
  
  setColor('glassColor', s.glassColor);
  setColor('fontColor', s.fontColor);
  setColor('fillColor', s.fillColor);
  setColor('subColor', s.subColor);
  setColor('titleColor', s.titleColor);
  setColor('menuColor', s.menuColor);
  setIfIdle('menuOpacity', String(s.menuOpacity));
  setIfIdle('menuOpacityNum', String(s.menuOpacity));
  setColor('iconColor', s.iconColor);
  setColor('romColor', s.romColor);
  setColor('outlineColor', s.outlineColor);

  // Outline size 0 means "no outline".
  const ow = s.outline ? s.outlineWidth : 0;
  setIfIdle('outlineWidth', String(ow));
  setIfIdle('outlineWidthNum', String(ow));
  el<HTMLInputElement>('karaoke').checked = s.karaoke;
  el<HTMLInputElement>('romanize').checked = s.romanize;
  el<HTMLInputElement>('clickThrough').checked = s.clickThrough;
  el<HTMLInputElement>('dynamicColor').checked = s.dynamicColor;
  el<HTMLSelectElement>('translate').value = s.dualSubtitles ? s.translateTo : 'off';
  el<HTMLInputElement>('autostart').checked = s.startWithWindows;
  el<HTMLInputElement>('idleFade').checked = s.idleFade;
  el<HTMLInputElement>('onTop').checked = s.alwaysOnTop;

  for (const [id, cur] of [['align', s.align], ['lines', String(s.lines)]] as const) {
    el(id).querySelectorAll('button').forEach((b) => b.classList.toggle('on', b.dataset.v === cur));
  }

  const off = media ? ((s.offsets && s.offsets[media.key]) ?? 0) / 1000 : 0;
  el('songOffset').textContent = off === 0 ? '0' : `${off > 0 ? '+' : ''}${+off.toFixed(2)}s`;
}

function showMedia() {
  el('nowTitle').textContent = media ? media.title : 'Nothing playing';
  el('nowArtist').textContent = media
    ? media.artist || 'Unknown artist'
    : 'Start a song in Spotify, YouTube Music or any player';
  fitMarquee();
}

/** A title that is too long for its row slides left until the last word is visible, then back. */
function fitMarquee() {
  const box = el('nowTitleBox');
  const title = el('nowTitle');
  box.classList.remove('run');
  const over = title.offsetWidth - box.clientWidth;
  if (over > 2) {
    box.style.setProperty('--shift', `${-over}px`);
    box.style.setProperty('--dur', `${Math.max(6, over / 28 + 4).toFixed(1)}s`);
    box.classList.add('run');
  }
}
new ResizeObserver(() => fitMarquee()).observe(el('nowTitleBox'));

/** The settings window is exactly as tall as its content: it ends right below the credit line. */
let fittedH = 0;
new ResizeObserver(() => {
  const h = Math.ceil(app.getBoundingClientRect().height);
  if (h < 200 || h === fittedH) return;
  fittedH = h;
  void getCurrentWindow().setSize(new LogicalSize(window.innerWidth, h)).catch(() => {});
}).observe(app);

const bind = <K extends keyof Settings>(id: string, key: K, read: (i: HTMLInputElement) => Settings[K]) => {
  const target = el<HTMLInputElement>(id);
  target.addEventListener('input', () => patch({ [key]: read(target) } as Partial<Settings>));
  target.addEventListener('change', () => patch({ [key]: read(target) } as Partial<Settings>));
};


/** A slider with a number box next to it: both show the current value and edit the same setting. */
function bindNumber(
  rangeId: string,
  numId: string,
  min: number,
  max: number,
  apply: (v: number) => Partial<Settings>,
  current: () => number,
) {
  const range = el<HTMLInputElement>(rangeId);
  const num = el<HTMLInputElement>(numId);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  range.addEventListener('input', () => {
    const v = Number(range.value);
    num.value = String(v);
    patch(apply(v));
  });
  // While typing, only act on values that are already valid ("3" on the way to "30" is ignored).
  num.addEventListener('input', () => {
    const v = Number(num.value);
    if (num.value === '' || !Number.isFinite(v) || v < min || v > max) return;
    range.value = String(v);
    patch(apply(v));
  });
  num.addEventListener('change', () => {
    const raw = num.value === '' ? current() : Number(num.value);
    const v = clamp(Number.isFinite(raw) ? raw : current());
    num.value = String(v);
    range.value = String(v);
    patch(apply(v));
  });
}

bindNumber('opacity', 'opacityNum', 0, 100, (v) => ({ opacity: Math.round(v) }), () => s.opacity);
bindNumber('menuOpacity', 'menuOpacityNum', 0, 100, (v) => ({ menuOpacity: Math.round(v) }), () => s.menuOpacity);
bindNumber('fontPx', 'fontPxNum', 12, 120, (v) => ({ fontPx: Math.round(v) }), () => s.fontPx);
bindNumber('subFontPx', 'subFontPxNum', 8, 80, (v) => ({ subFontPx: Math.round(v) }), () => s.subFontPx);
bindNumber('romFontPx', 'romFontPxNum', 8, 80, (v) => ({ romFontPx: Math.round(v) }), () => s.romFontPx);
bindNumber('outlineWidth', 'outlineWidthNum', 0, 6, (v) => ({ outlineWidth: v, outline: v > 0 }), () => (s.outline ? s.outlineWidth : 0));

const colorBusyUntil: Record<string, number> = {};

function setColor(id: string, value: string) {
  const input = el<HTMLInputElement>(id);
  if (document.activeElement === input || performance.now() < (colorBusyUntil[id] ?? 0)) return;
  if (input.value.toLowerCase() !== value.toLowerCase()) input.value = value;
}

const bindColor = (id: string, key: 'glassColor' | 'fontColor' | 'fillColor' | 'outlineColor' | 'subColor' | 'romColor' | 'titleColor' | 'iconColor' | 'menuColor') => {
  const input = el<HTMLInputElement>(id);
  let timer = 0;
  input.addEventListener('focus', () => {
    colorBusyUntil[id] = Number.POSITIVE_INFINITY;
  });
  input.addEventListener('blur', () => {
    colorBusyUntil[id] = performance.now() + 1000;
  });
  const send = (finalize = false) => {
    clearTimeout(timer);
    timer = 0;
    const value = input.value;
    if (s[key].toLowerCase() === value.toLowerCase() && !(key === 'fillColor' && s.dynamicColor)) {
      if (finalize) colorBusyUntil[id] = performance.now() + 1000;
      return;
    }
    if (key === 'fillColor' && s.dynamicColor) {
      s.dynamicColor = false;
      el<HTMLInputElement>('dynamicColor').checked = false;
    }
    s[key] = value;
    colorBusyUntil[id] = finalize ? performance.now() + 1000 : Number.POSITIVE_INFINITY;
    patch(key === 'fillColor' && !s.dynamicColor
      ? { [key]: value, dynamicColor: false }
      : { [key]: value });
  };
  // Native color dialogs can emit rapid input events while the user drags in the picker.
  input.addEventListener('input', () => {
    colorBusyUntil[id] = Number.POSITIVE_INFINITY;
    if (!timer) timer = window.setTimeout(send, 120);
  });
  input.addEventListener('change', () => {
    send(true);
  });
};

bindColor('fontColor', 'fontColor');
bindColor('glassColor', 'glassColor');
bindColor('fillColor', 'fillColor');
bindColor('outlineColor', 'outlineColor');
bindColor('subColor', 'subColor');
bindColor('romColor', 'romColor');
bindColor('titleColor', 'titleColor');
bindColor('menuColor', 'menuColor');
bindColor('iconColor', 'iconColor');

bind('karaoke', 'karaoke', (i) => i.checked);
bind('romanize', 'romanize', (i) => i.checked);
bind('clickThrough', 'clickThrough', (i) => i.checked);
bind('onTop', 'alwaysOnTop', (i) => i.checked);
bind('dynamicColor', 'dynamicColor', (i) => i.checked);
bind('autostart', 'startWithWindows', (i) => i.checked);
bind('idleFade', 'idleFade', (i) => i.checked);
el<HTMLSelectElement>('translate').addEventListener('change', (e) => {
  const v = (e.target as HTMLSelectElement).value;
  if (v === 'off') patch({ dualSubtitles: false });
  else patch({ dualSubtitles: true, translateTo: v });
});

el<HTMLSelectElement>('gameMode').addEventListener('change', (e) =>
  patch({ gameMode: (e.target as HTMLSelectElement).value as Settings['gameMode'] }));
el<HTMLSelectElement>('fontFamily').addEventListener('change', (e) =>
  patch({ fontFamily: (e.target as HTMLSelectElement).value as Settings['fontFamily'] }));
el('align').addEventListener('click', (e) => {
  const v = (e.target as HTMLElement).closest('button')?.dataset.v;
  if (v) patch({ align: v as Settings['align'] });
});
el('lines').addEventListener('click', (e) => {
  const v = (e.target as HTMLElement).dataset.v;
  if (v) patch({ lines: Number(v) as Settings['lines'] });
});

el('resetOffset').addEventListener('click', () => media && void api.setOffset(media.key, 0));
el('hide').addEventListener('click', () => void api.hideSettings());
el('quit').addEventListener('click', () => void api.quit());

// ---- manual lyric search ----------------------------------------------------------

const fmt = (sec: number) => `${Math.floor(sec / 60)}:${String(Math.round(sec % 60)).padStart(2, '0')}`;

async function runSearch() {
  const q = el<HTMLInputElement>('query').value.replace(/\s*\(lyrics\)/gi, '').trim();
  const box = el('results');
  if (!q) return;
  box.innerHTML = '<p class="hint">Searching…</p>';
  try {
    const found = await api.searchLyrics(q);
    // Closest to the length of the song that is playing first; untimed (plain-text) versions last.
    const want = media && media.durationMs > 0 ? media.durationMs / 1000 : 0;
    const gap = (l: Lyrics) => (l.approx ? 1e6 : 0) + (want && l.duration ? Math.abs(l.duration - want) : 1e3);
    const list = want ? [...found].sort((a, b) => gap(a) - gap(b)) : found;
    box.replaceChildren();
    if (list.length === 0) {
      box.innerHTML = '<p class="hint">No synced lyrics found for that search.</p>';
      return;
    }
    list.forEach((l: Lyrics) => {
      const row = document.createElement('div');
      row.className = 'result';
      const info = document.createElement('div');
      const title = document.createElement('b');
      title.textContent = l.instrumental ? `${l.title} (instrumental)` : l.title;
      const sub = document.createElement('small');
      const off = want && l.duration ? Math.round(Math.abs(l.duration - want)) : null;
      const len = off === null ? '' : off === 0 ? ' · same length' : ` · ${off}s off`;
      sub.textContent = `${l.artist} · ${l.duration ? fmt(l.duration) : '?'}${len} · ${l.lines.length} lines${l.approx ? ' · untimed' : ''}`;
      info.append(title, sub);
      const use = document.createElement('button');
      use.textContent = 'Use';
      use.disabled = !media;
      use.addEventListener('click', () => {
        if (!media) return;
        void api.applyLyrics(media.key, l);
        use.textContent = 'Applied';
      });
      row.append(info, use);
      box.append(row);
    });
  } catch {
    box.innerHTML = "<p class=\"hint\">Couldn't reach the lyrics service.</p>";
  }
}

el('search').addEventListener('click', () => void runSearch());
el('query').addEventListener('keydown', (e) => e.key === 'Enter' && void runSearch());

// Show the other versions for the song that is playing as soon as Settings is looked at.
let searchedFor = '';
function autoSearch() {
  if (!media || searchedFor === media.key) return;
  searchedFor = media.key;
  el<HTMLInputElement>('query').value = `${media.title} ${media.artist}`.trim();
  void runSearch();
}
window.addEventListener('focus', autoSearch);

// ---- version + update check ----------------------------------------------------------

async function showVersion() {
  const box = el('version');
  try {
    box.textContent = `v${await api.appVersion()}`;
    const u = await api.checkUpdate();
    if (u.newer) {
      const link = document.createElement('button');
      link.className = 'update';
      link.textContent = `${u.latest} available`;
      link.title = 'Open the download page';
      link.addEventListener('click', () => void api.openReleasePage(u.url));
      box.append(' · ', link);
    }
  } catch {
    /* offline or rate-limited: the version number alone is fine */
  }
}
void showVersion();

// ---- boot ---------------------------------------------------------------------------

void (async () => {
  s = await api.getSettings();
  sync();
  await on<Settings>('settings-changed', (next) => {
    s = next;
    sync();
  });
  await on<Media | null>('media', (m) => {
    const changed = (m?.key ?? '') !== (media?.key ?? '');
    media = m;
    if (changed) {
      showMedia();
      if (m) el<HTMLInputElement>('query').value = `${m.title} ${m.artist}`.trim();
      el('results').replaceChildren();
      searchedFor = '';
      sync();
      if (document.hasFocus()) autoSearch();
    }
  });
})();
