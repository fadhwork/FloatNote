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
}

export interface Line {
  t: number;
  text: string;
}

export interface Lyrics {
  lines: Line[];
  instrumental: boolean;
  source: string;
  title: string;
  artist: string;
  duration: number;
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
  quit: () => invoke<void>('quit_app'),
};

export const on = <T>(event: string, cb: (payload: T) => void) =>
  listen<T>(event, (e) => cb(e.payload));
