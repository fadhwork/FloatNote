/** Query `?preview` enables a browser-only overlay preview without invoking Tauri. */
export const previewMode = typeof window !== 'undefined' && new URLSearchParams(location.search).has('preview');

type Settings = {
  opacity: number; glassColor: string; fontPx: number; fontColor: string; fillColor: string;
  subColor: string; subFontPx: number; romColor: string; romFontPx: number; outline: boolean;
  outlineColor: string; outlineWidth: number; fontFamily: 'sans' | 'display' | 'serif' | 'mono';
  align: 'left' | 'center' | 'right'; lines: 1 | 3 | 5; karaoke: boolean; clickThrough: boolean;
  alwaysOnTop: boolean; firstRunDone: boolean; dynamicColor: boolean; dualSubtitles: boolean;
  romanize: boolean; translateTo: string; startWithWindows: boolean; idleFade: boolean;
  offsets: Record<string, number>;
};
type Media = {
  key: string; title: string; artist: string; album: string; source: string; playing: boolean;
  positionMs: number; durationMs: number; clock?: string;
};
type Lyrics = {
  lines: { t: number; text: string; sub?: string; rom?: string }[]; instrumental: boolean;
  source: string; title: string; artist: string; duration: number; approx?: boolean;
};

const defaults: Settings = {
  opacity: 35, glassColor: '#24283a', fontPx: 30, fontColor: '#ffffff', fillColor: '#7dd3fc',
  subColor: '#b8e7ff', subFontPx: 14, romColor: '#ffffff', romFontPx: 16, outline: true,
  outlineColor: '#000000', outlineWidth: 2, fontFamily: 'sans', align: 'center', lines: 3,
  karaoke: true, clickThrough: false, alwaysOnTop: true, firstRunDone: true, dynamicColor: false,
  dualSubtitles: true, romanize: false, translateTo: 'en', startWithWindows: false, idleFade: false, offsets: {},
};
export const previewSettings: Settings = { ...defaults };
export const previewMedia: Media = {
  key: 'preview-midnight-city', title: 'Midnight City', artist: 'M83', album: 'Hurry Up, We’re Dreaming',
  source: 'FloatNote Preview', playing: true, positionMs: 38_000, durationMs: 244_000, clock: 'own',
};
const sampleLyrics: Lyrics = {
  source: 'preview', title: previewMedia.title, artist: previewMedia.artist, duration: 244, instrumental: false,
  lines: [
    { t: 0, text: 'Waiting in the car', sub: '待ち続けている', rom: 'Machitsuzukete iru' },
    { t: 5, text: 'Waiting for the ride in the dark', sub: '暗闇の中で', rom: 'Kurayami no naka de' },
    { t: 10, text: 'At night the city grows', sub: '夜に街は広がる', rom: 'Yoru ni machi wa hirogaru' },
    { t: 15, text: 'Look and see her eyes, they glow', sub: '輝く瞳を見つめて', rom: 'Kagayaku hitomi o mitsumete' },
    { t: 20, text: 'The city is my church', sub: 'この街が僕の教会', rom: 'Kono machi ga boku no kyoukai' },
    { t: 25, text: 'It wraps me in its blinding twilight', sub: '眩しい夕暮れに包まれる', rom: 'Mabushii yuugure ni tsutsumareru' },
    { t: 30, text: 'I hear the voices calling', sub: '呼ぶ声が聞こえる', rom: 'Yobu koe ga kikoeru' },
    { t: 35, text: 'I hear them from the rooftops', sub: '屋上から聞こえてくる', rom: 'Okujou kara kikoete kuru' },
    { t: 40, text: 'Neon lights are shining', sub: 'ネオンが輝いている', rom: 'Neon ga kagayaite iru' },
    { t: 45, text: 'The night is ours tonight', sub: '今夜は僕らの夜', rom: 'Konya wa bokura no yoru' },
    { t: 50, text: 'Waiting in the car', sub: '待ち続けている', rom: 'Machitsuzukete iru' },
    { t: 55, text: 'Waiting for the ride in the dark', sub: '暗闇の中で', rom: 'Kurayami no naka de' },
  ],
};
export function getPreviewLyrics(): Lyrics { return structuredClone(sampleLyrics); }

export async function previewInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  let result: unknown;
  switch (command) {
    case 'get_settings': result = { ...previewSettings, offsets: { ...previewSettings.offsets } }; break;
    case 'update_settings':
      Object.assign(previewSettings, (args?.patch ?? {}) as Partial<Settings>);
      window.dispatchEvent(new CustomEvent('floatnote-preview-settings', { detail: { ...previewSettings } }));
      result = { ...previewSettings, offsets: { ...previewSettings.offsets } }; break;
    case 'set_offset': {
      const key = String(args?.key ?? previewMedia.key); const ms = Number(args?.ms ?? 0);
      if (ms === 0) delete previewSettings.offsets[key]; else previewSettings.offsets[key] = ms;
      result = undefined; break;
    }
    case 'get_lyrics': result = getPreviewLyrics(); break;
    case 'search_lyrics': result = [getPreviewLyrics()]; break;
    case 'apply_lyrics': {
      const lyrics = args?.lyrics as Lyrics | undefined;
      if (lyrics) window.dispatchEvent(new CustomEvent('floatnote-preview-lyrics', { detail: { key: args?.key, lyrics } }));
      result = undefined; break;
    }
    case 'get_album_art': result = null; break;
    case 'translate_lyrics': case 'romanize_lyrics': result = ((args?.lines as string[]) ?? []).map(() => ''); break;
    default: result = undefined;
  }
  return result as T;
}

export function previewListen<T>(event: string, callback: (payload: T) => void): (() => void) | null {
  if (!previewMode) return null;
  const settingsListener = (e: Event) => callback((e as CustomEvent).detail as T);
  const lyricsListener = (e: Event) => callback((e as CustomEvent).detail as T);
  const mediaTimer = event === 'media' ? window.setInterval(() => {
    previewMedia.positionMs = (previewMedia.positionMs + 150) % previewMedia.durationMs;
    callback({ ...previewMedia } as T);
  }, 150) : 0;
  if (event === 'settings-changed') window.addEventListener('floatnote-preview-settings', settingsListener);
  if (event === 'lyrics-override') window.addEventListener('floatnote-preview-lyrics', lyricsListener);
  if (event === 'hover') queueMicrotask(() => callback(true as T));
  if (event === 'media') queueMicrotask(() => callback({ ...previewMedia } as T));
  return () => {
    if (event === 'settings-changed') window.removeEventListener('floatnote-preview-settings', settingsListener);
    if (event === 'lyrics-override') window.removeEventListener('floatnote-preview-lyrics', lyricsListener);
    if (mediaTimer) window.clearInterval(mediaTimer);
  };
}
