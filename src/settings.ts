import { api, on, type Lyrics, type Media, type Settings } from './api';
import './settings.css';

const app = document.getElementById('app')!;
app.innerHTML = `
<header>
  <h1>FloatNote</h1>
  <p>Changes apply instantly. Close this window to tuck FloatNote into the tray.</p>
</header>

<section>
  <h2>Now playing</h2>
  <div class="now"><b id="nowTitle">Nothing playing</b><span id="nowArtist">Start a song in Spotify, YouTube Music or any player</span></div>
</section>

<section>
  <h2>Appearance</h2>
  <div class="row"><label for="opacity">Glass opacity</label>
    <div class="ctl"><input id="opacity" type="range" min="0" max="100" /><span class="val" id="opacityVal"></span></div></div>
  <div class="row"><span class="lbl">Text size</span>
    <div class="ctl"><button class="sq" id="fontMinus" aria-label="Smaller">−</button><input id="fontPx" type="number" min="12" max="120" /><button class="sq" id="fontPlus" aria-label="Larger">+</button></div></div>
  <div class="row"><label for="fontFamily">Font</label>
    <div class="ctl"><select id="fontFamily"><option value="sans">Segoe (clean)</option><option value="display">Bahnschrift (bold)</option><option value="serif">Serif</option><option value="mono">Monospace</option></select></div></div>
  <div class="row"><span class="lbl">Alignment</span><div class="ctl"><div class="seg" id="align"><button data-v="left">Left</button><button data-v="center">Center</button><button data-v="right">Right</button></div></div></div>
  <div class="row"><span class="lbl">Lines shown</span><div class="ctl"><div class="seg" id="lines"><button data-v="1">1</button><button data-v="3">3</button><button data-v="5">5</button></div></div></div>
  <div class="row"><label for="fontColor">Text colour</label><div class="ctl"><input id="fontColor" type="color" /></div></div>
  <div class="row"><label for="dynamicColor">Dynamic album accent</label><div class="ctl"><label class="switch"><input id="dynamicColor" type="checkbox" /><span></span></label></div></div>  <div class="row"><label for="karaoke">Karaoke sweep</label><div class="ctl"><input id="fillColor" type="color" aria-label="Sweep colour" /><label class="switch"><input id="karaoke" type="checkbox" /><span></span></label></div></div>
  <div class="row"><label for="outline">Text outline</label><div class="ctl"><input id="outlineColor" type="color" aria-label="Outline colour" /><input id="outlineWidth" type="range" min="0" max="6" step="0.5" style="max-width:90px" /><label class="switch"><input id="outline" type="checkbox" /><span></span></label></div></div>
</section>

<section>
  <h2>Behaviour</h2>
  <div class="row"><label for="clickThrough">Click-through (lock)</label><div class="ctl"><label class="switch"><input id="clickThrough" type="checkbox" /><span></span></label></div></div>
  <p class="hint">While locked, clicks pass straight through the lyrics. Only the drag handle, resize corner and timing buttons respond. Press <b>Ctrl+Alt+L</b> or use the tray icon to unlock.</p>
  <div class="row"><label for="dualSubtitles">Dual subtitles (Translate)</label><div class="ctl"><label class="switch"><input id="dualSubtitles" type="checkbox" /><span></span></label></div></div>  <div class="row"><label for="onTop">Always on top</label><div class="ctl"><label class="switch"><input id="onTop" type="checkbox" /><span></span></label></div></div>
</section>

<section>
  <h2>Lyrics</h2>
  <div class="row"><span class="lbl">Timing for this song</span><div class="ctl"><span class="val" id="songOffset">0.00s</span><button id="resetOffset">Reset</button></div></div>
  <div class="row"><input id="query" type="text" placeholder="Wrong lyrics? Search title and artist" /><button class="primary" id="search">Search</button></div>
  <div id="results"></div>
</section>

<footer><button id="hide">Hide to tray</button><button class="danger" id="quit">Quit</button></footer><div class="author-footer">FloatNote · crafted by fadhlihsani</div>
`;

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
let s: Settings;
let media: Media | null = null;

// Coalesce rapid slider input into one update per frame.
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
  el<HTMLInputElement>('opacity').value = String(s.opacity);
  el('opacityVal').textContent = `${s.opacity}%`;
  el<HTMLInputElement>('fontPx').value = String(s.fontPx);
  el<HTMLSelectElement>('fontFamily').value = s.fontFamily;
  el<HTMLInputElement>('fontColor').value = s.fontColor;
  el<HTMLInputElement>('fillColor').value = s.fillColor;
  el<HTMLInputElement>('outlineColor').value = s.outlineColor;
  el<HTMLInputElement>('outlineWidth').value = String(s.outlineWidth);
  el<HTMLInputElement>('outline').checked = s.outline;
  el<HTMLInputElement>('karaoke').checked = s.karaoke;
  el<HTMLInputElement>('clickThrough').checked = s.clickThrough;
  el<HTMLInputElement>('dynamicColor').checked = s.dynamicColor;
  el<HTMLInputElement>('dualSubtitles').checked = s.dualSubtitles;
  el<HTMLInputElement>('onTop').checked = s.alwaysOnTop;
  for (const [id, cur] of [['align', s.align], ['lines', String(s.lines)]] as const) {
    el(id).querySelectorAll('button').forEach((b) => b.classList.toggle('on', b.dataset.v === cur));
  }
  const off = media ? (s.offsets[media.key] ?? 0) / 1000 : 0;
  el('songOffset').textContent = `${off > 0 ? '+' : ''}${off.toFixed(2)}s`;
}

function showMedia() {
  el('nowTitle').textContent = media ? media.title : 'Nothing playing';
  el('nowArtist').textContent = media
    ? media.artist || 'Unknown artist'
    : 'Start a song in Spotify, YouTube Music or any player';
}

const bind = <K extends keyof Settings>(id: string, key: K, read: (i: HTMLInputElement) => Settings[K]) =>
  el<HTMLInputElement>(id).addEventListener('input', (e) => patch({ [key]: read(e.target as HTMLInputElement) } as Partial<Settings>));

bind('opacity', 'opacity', (i) => Number(i.value));
bind('fontPx', 'fontPx', (i) => Number(i.value));
bind('fontColor', 'fontColor', (i) => i.value);
bind('fillColor', 'fillColor', (i) => i.value);
bind('outlineColor', 'outlineColor', (i) => i.value);
bind('outlineWidth', 'outlineWidth', (i) => Number(i.value));
bind('outline', 'outline', (i) => i.checked);
bind('karaoke', 'karaoke', (i) => i.checked);
bind('clickThrough', 'clickThrough', (i) => i.checked);
bind('onTop', 'alwaysOnTop', (i) => i.checked);
bind('dynamicColor', 'dynamicColor', (i) => i.checked);
bind('dualSubtitles', 'dualSubtitles', (i) => i.checked);
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
