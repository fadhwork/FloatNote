import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface Settings {
  opacity: number;
  fontPx: number;
  fontColor: string;
  fillColor: string;
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
  getSettings: () => invoke<Settings>('get_settings'),
  update: (patch: Partial<Settings>) => invoke<Settings>('update_settings', { patch }),
  setHitRegions: (rects: Rect[]) => invoke<void>('set_hit_regions', { rects }),
  setOffset: (key: string, ms: number) => invoke<void>('set_offset', { key, ms }),
  getLyrics: (m: Media) =>
    invoke<Lyrics>('get_lyrics', {
      key: m.key,
      title: m.title,
      artist: m.artist,
      album: m.album,
      durationMs: m.durationMs,
    }),
  searchLyrics: (query: string) => invoke<Lyrics[]>('search_lyrics', { query }),
  applyLyrics: (key: string, lyrics: Lyrics) => invoke<void>('apply_lyrics', { key, lyrics }),
  openSettings: () => invoke<void>('open_settings'),
  hideSettings: () => invoke<void>('hide_settings'),
  mediaControl: (action: 'toggle' | 'next' | 'prev') => invoke<void>('media_control', { action }),
  getAlbumArt: (title: string, artist: string) => invoke<string | null>('get_album_art', { title, artist }),
  /** One translated string per input line ("" = nothing to show). Cached on disk per song. */
  translateLyrics: (key: string, lines: string[], target: string) =>
    invoke<string[]>('translate_lyrics', { key, lines, target }),
  romanizeLyrics: (key: string, lines: string[]) =>
    invoke<string[]>('romanize_lyrics', { key, lines }),
  quit: () => invoke<void>('quit_app'),
};

export const on = <T>(event: string, cb: (payload: T) => void) =>
  listen<T>(event, (e) => cb(e.payload));
