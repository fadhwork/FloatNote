import { api, on, type Lyrics, type Media, type Settings } from './api';
import './settings.css';

const app = document.getElementById('app')!;
app.innerHTML = `
<header class="top">
  <div class="np"><b id="nowTitle">Nothing playing</b><span id="nowArtist">Start a song in Spotify, YouTube Music or any player</span></div>
  <div class="brand">FloatNote</div>
</header>

<div class="cols">
  <div class="col">
    <section>
      <h2>Appearance</h2>
      <div class="row"><label for="opacity">Glass opacity</label>
        <div class="ctl"><input id="opacity" type="range" min="0" max="100" /><span class="val" id="opacityVal">35%</span></div></div>
      <div class="row"><span class="lbl">Text size</span>
        <div class="ctl"><button class="sq" id="fontMinus" aria-label="Smaller">−</button><input id="fontPx" type="range" min="14" max="72" /><button class="sq" id="fontPlus" aria-label="Larger">+</button></div></div>
      <div class="row"><label for="fontFamily">Font</label>
        <div class="ctl"><select id="fontFamily"><option value="sans">Modern Sans</option><option value="display">Display</option><option value="serif">Editorial Serif</option><option value="mono">Monospace</option></select></div></div>
      <div class="row"><span class="lbl">Align</span><div class="ctl"><div class="seg" id="align"><button data-v="left">Left</button><button data-v="center">Center</button><button data-v="right">Right</button></div></div></div>
      <div class="row"><span class="lbl">Lines</span><div class="ctl"><div class="seg" id="lines"><button data-v="1">1</button><button data-v="3">3</button><button data-v="5">5</button></div></div></div>
      <div class="row colors">
        <label class="sw">Text <input id="fontColor" type="color" /></label>
        <label class="sw">Sweep <input id="fillColor" type="color" aria-label="Sweep colour" /></label>
        <label class="sw">Outline <input id="outlineColor" type="color" aria-label="Outline colour" /></label>
      </div>
      <div class="row"><label for="outline">Outline</label>
        <div class="ctl"><input id="outlineWidth" type="range" min="0" max="6" step="0.5" /><label class="switch"><input id="outline" type="checkbox" /><span></span></label></div></div>
      <div class="row pair">
        <label class="half" for="karaoke">Karaoke sweep<span class="switch"><input id="karaoke" type="checkbox" /><span></span></span></label>
        <label class="half" for="dynamicColor">Album accent<span class="switch"><input id="dynamicColor" type="checkbox" /><span></span></span></label>
      </div>
    </section>
  </div>

  <div class="col">
    <section>
      <h2>Behaviour</h2>
      <div class="row"><label for="clickThrough">Click-through (lock)</label><div class="ctl"><label class="switch"><input id="clickThrough" type="checkbox" /><span></span></label></div></div>
      <p class="hint">Locked: only the handle, resize corner, pause and timing respond. <b>Ctrl+Alt+L</b> unlocks.</p>
      <div class="row"><label for="onTop">Always on top</label><div class="ctl"><label class="switch"><input id="onTop" type="checkbox" /><span></span></label></div></div>
      <div class="row"><label for="autostart">Start with Windows</label><div class="ctl"><label class="switch"><input id="autostart" type="checkbox" /><span></span></label></div></div>
      <div class="row"><label for="idleFade">Fade when idle</label><div class="ctl"><label class="switch"><input id="idleFade" type="checkbox" /><span></span></label></div></div>
      <div class="row"><label for="translate">Translate lyrics</label>
        <div class="ctl"><select id="translate"><option value="off">Off</option><option value="en">English</option><option value="id">Indonesia</option><option value="es">Español</option><option value="fr">Français</option><option value="de">Deutsch</option><option value="pt">Português</option><option value="ja">日本語</option><option value="ko">한국어</option><option value="zh-CN">中文</option></select></div></div>
    </section>

    <section>
      <h2>Lyrics</h2>
      <div class="row"><span class="lbl">Timing, this song</span><div class="ctl"><span class="val" id="songOffset">0s</span><button id="resetOffset">Reset</button></div></div>
      <div class="row"><input id="query" type="text" placeholder="Wrong lyrics? Search title, artist" /><button class="primary" id="search">Search</button></div>
      <div id="results"></div>
    </section>
  </div>
</div>

<footer>
  <span class="credit">crafted by fadhlihsani with love</span>
  <span class="btns"><button id="hide">Hide to tray</button><button class="danger" id="quit">Quit</button></span>
</footer>
`;

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let s: Settings = {
  opacity: 35,
  fontPx: 30,
  fontColor: '#ffffff',
  fillColor: '#7dd3fc',
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
  translateTo: 'en',
  startWithWindows: false,
  idleFade: true,
  offsets: {},
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

function sync() {
  if (!s) return;
  el<HTMLInputElement>('opacity').value = String(s.opacity);
  el('opacityVal').textContent = `${s.opacity}%`;
  el<HTMLInputElement>('fontPx').value = String(s.fontPx);
  el<HTMLSelectElement>('fontFamily').value = s.fontFamily;
  
  setColor('fontColor', s.fontColor);
  setColor('fillColor', s.fillColor);
  setColor('outlineColor', s.outlineColor);

  el<HTMLInputElement>('outlineWidth').value = String(s.outlineWidth);
  el<HTMLInputElement>('outline').checked = s.outline;
  el<HTMLInputElement>('karaoke').checked = s.karaoke;
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
}

const bind = <K extends keyof Settings>(id: string, key: K, read: (i: HTMLInputElement) => Settings[K]) => {
  const target = el<HTMLInputElement>(id);
  target.addEventListener('input', () => patch({ [key]: read(target) } as Partial<Settings>));
  target.addEventListener('change', () => patch({ [key]: read(target) } as Partial<Settings>));
};

bind('opacity', 'opacity', (i) => Number(i.value));
bind('fontPx', 'fontPx', (i) => Number(i.value));

// While the colour popup / eyedropper is in use, settings echoes from the backend are stale
// (they carry colours from a moment ago). Writing them back into the input resets the picker,
// so colour inputs are left alone until ~0.6 s after the last change.
const colorBusyUntil: Record<string, number> = {};

function setColor(id: string, value: string) {
  if (performance.now() < (colorBusyUntil[id] ?? 0)) return;
  const input = el<HTMLInputElement>(id);
  if (input.value.toLowerCase() !== value.toLowerCase()) input.value = value;
}

const bindColor = (id: string, key: 'fontColor' | 'fillColor' | 'outlineColor') => {
  const input = el<HTMLInputElement>(id);
  let timer = 0;
  const send = () => {
    clearTimeout(timer);
    timer = 0;
    if (key === 'fillColor' && s && s.dynamicColor) {
      s.dynamicColor = false;
      el<HTMLInputElement>('dynamicColor').checked = false;
      patch({ dynamicColor: false, fillColor: input.value });
      return;
    }
    patch({ [key]: input.value });
  };
  // The eyedropper fires `input` on every mouse move: send at most ~8 updates per second.
  input.addEventListener('input', () => {
    colorBusyUntil[id] = performance.now() + 600;
    if (!timer) timer = window.setTimeout(send, 120);
  });
  input.addEventListener('change', () => {
    colorBusyUntil[id] = performance.now() + 600;
    send();
  });
};

bindColor('fontColor', 'fontColor');
bindColor('fillColor', 'fillColor');
bindColor('outlineColor', 'outlineColor');

bind('outlineWidth', 'outlineWidth', (i) => Number(i.value));
bind('outline', 'outline', (i) => i.checked);
bind('karaoke', 'karaoke', (i) => i.checked);
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

el<HTMLSelectElement>('fontFamily').addEventListener('change', (e) =>
  patch({ fontFamily: (e.target as HTMLSelectElement).value as Settings['fontFamily'] }));
el('fontMinus').addEventListener('click', () => patch({ fontPx: Math.max(12, s.fontPx - 2) }));
el('fontPlus').addEventListener('click', () => patch({ fontPx: Math.min(120, s.fontPx + 2) }));
el('align').addEventListener('click', (e) => {
  const v = (e.target as HTMLElement).dataset.v;
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
  const q = el<HTMLInputElement>('query').value.trim();
  const box = el('results');
  if (!q) return;
  box.innerHTML = '<p class="hint">Searching…</p>';
  try {
    const list = await api.searchLyrics(q);
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
      sub.textContent = `${l.artist} · ${l.duration ? fmt(l.duration) : '?'} · ${l.lines.length} lines`;
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
      sync();
    }
  });
})();
