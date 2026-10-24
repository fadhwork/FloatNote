import { previewInvoke, previewListen, previewMode } from './preview';
const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
  if (previewMode) return previewInvoke<T>(command, args);
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
};

export interface Settings {
  opacity: number;
  glassColor: string;
  fontPx: number;
  fontColor: string;
  fillColor: string;
  subColor: string;
  subFontPx: number;
  romColor: string;
  romFontPx: number;
  outline: boolean;
  outlineColor: string;
  outlineWidth: number;
  fontFamily: 'sans' | 'display' | 'serif' | 'mono';
  align: 'left' | 'center' | 'right';
  lines: 1 | 3 | 5;
  karaoke: boolean;
  clickThrough: boolean;
  alwaysOnTop: boolean;
  firstRunDone: boolean;
  dynamicColor: boolean;
  dualSubtitles: boolean;
  /** Show a Latin-letter reading under non-Latin lyrics (kanji, kana, hangul, ...). */
  romanize: boolean;
  /** Language of the second subtitle line (en, id, es, fr, de, pt, ja, ko, zh-CN). */
  translateTo: string;
  startWithWindows: boolean;
  idleFade: boolean;
  offsets: Record<string, number>;
}

export interface Media {
  key: string;
  title: string;
  artist: string;
  album: string;
  source: string;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  /** "player" = position reported by the player, "own" = FloatNote's own clock. */
  clock?: string;
}

export interface Line {
  t: number;
  text: string;
  sub?: string;
  /** Latin-letter reading of `text` (for singing along). */
  rom?: string;
}

export interface Lyrics {
  lines: Line[];
  instrumental: boolean;
  source: string;
  title: string;
  artist: string;
  duration: number;
  /** true = no synced lyrics exist; line times are spread evenly over the song (a guess). */
  approx?: boolean;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const api = {
  getSettings: () => call<Settings>('get_settings'),
  update: (patch: Partial<Settings>) => call<Settings>('update_settings', { patch }),
  setHitRegions: (rects: Rect[]) => call<void>('set_hit_regions', { rects }),
  setOffset: (key: string, ms: number) => call<void>('set_offset', { key, ms }),
  getLyrics: (m: Media, refresh = false) =>
    call<Lyrics>('get_lyrics', {
      key: m.key,
      title: m.title,
      artist: m.artist,
      album: m.album,
      durationMs: m.durationMs,
      refresh,
    }),
  searchLyrics: (query: string) => call<Lyrics[]>('search_lyrics', { query }),
  applyLyrics: (key: string, lyrics: Lyrics) => call<void>('apply_lyrics', { key, lyrics }),
  openSettings: () => call<void>('open_settings'),
  hideSettings: () => call<void>('hide_settings'),
  mediaControl: (action: 'toggle' | 'next' | 'prev') => call<void>('media_control', { action }),
  getAlbumArt: (title: string, artist: string) => call<string | null>('get_album_art', { title, artist }),
  /** One translated string per input line ("" = nothing to show). Cached on disk per song. */
  translateLyrics: (key: string, lines: string[], target: string) =>
    call<string[]>('translate_lyrics', { key, lines, target }),
  romanizeLyrics: (key: string, lines: string[]) =>
    call<string[]>('romanize_lyrics', { key, lines }),
  quit: () => call<void>('quit_app'),
};

export const on = <T>(event: string, cb: (payload: T) => void) => {
  if (previewMode) {
    const stop = previewListen<T>(event, cb);
    return Promise.resolve(stop ?? (() => {}));
  }
  return import('@tauri-apps/api/event').then(({ listen }) => listen<T>(event, (e) => cb(e.payload)));
};
