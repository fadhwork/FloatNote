import type { Settings } from './api';

/** The settings a "look" covers: appearance only, never behaviour, window position or per-song data. */
export const LOOK_KEYS = [
  'opacity', 'glassColor', 'fontPx', 'fontColor', 'fillColor', 'subColor', 'subFontPx', 'romColor',
  'romFontPx', 'outline', 'outlineColor', 'outlineWidth', 'fontFamily', 'align', 'lines', 'karaoke',
  'titleColor', 'iconColor', 'menuColor', 'menuOpacity',
] as const;

export type LookValues = Record<string, unknown>;

export interface Preset {
  name: string;
  values: LookValues;
}

export const BUILT_IN: Preset[] = [
  {
    name: 'midnight',
    values: {
      opacity: 35, glassColor: '#24283a', fontPx: 30, fontColor: '#ffffff', fillColor: '#7dd3fc',
      subColor: '#b8e7ff', subFontPx: 14, romColor: '#ffffff', romFontPx: 16, outline: true,
      outlineColor: '#000000', outlineWidth: 2, fontFamily: 'sans', align: 'center', lines: 3, karaoke: true,
      titleColor: '#b8e7ff', iconColor: '#ffffff',
      menuColor: '#24283a', menuOpacity: 55,
    },
  },
  {
    name: 'minimal',
    values: {
      opacity: 0, glassColor: '#000000', fontPx: 28, fontColor: '#ffffff', fillColor: '#a5f3fc',
      subColor: '#e5e7eb', subFontPx: 13, romColor: '#e5e7eb', romFontPx: 14, outline: true,
      outlineColor: '#000000', outlineWidth: 3, fontFamily: 'sans', align: 'center', lines: 1, karaoke: true,
      titleColor: '#e5e7eb', iconColor: '#ffffff',
      menuColor: '#000000', menuOpacity: 45,
    },
  },
  {
    name: 'night',
    values: {
      opacity: 55, glassColor: '#1a1033', fontPx: 38, fontColor: '#f5f3ff', fillColor: '#f472b6',
      subColor: '#c4b5fd', subFontPx: 16, romColor: '#fbcfe8', romFontPx: 18, outline: true,
      outlineColor: '#12002b', outlineWidth: 3, fontFamily: 'display', align: 'center', lines: 5, karaoke: true,
      titleColor: '#c4b5fd', iconColor: '#f5f3ff',
      menuColor: '#1a1033', menuOpacity: 60,
    },
  },
  {
    name: 'cinema',
    values: {
      opacity: 60, glassColor: '#0b0b0f', fontPx: 32, fontColor: '#f3e9d2', fillColor: '#f5c26b',
      subColor: '#d6c8a8', subFontPx: 14, romColor: '#e6d7b8', romFontPx: 16, outline: true,
      outlineColor: '#000000', outlineWidth: 2, fontFamily: 'serif', align: 'center', lines: 3, karaoke: true,
      titleColor: '#d6c8a8', iconColor: '#f3e9d2',
      menuColor: '#0b0b0f', menuOpacity: 65,
    },
  },
  {
    name: 'light',
    values: {
      opacity: 80, glassColor: '#f5f1e8', fontPx: 30, fontColor: '#1f2430', fillColor: '#2563eb',
      subColor: '#475569', subFontPx: 14, romColor: '#334155', romFontPx: 16, outline: false,
      outlineColor: '#ffffff', outlineWidth: 0, fontFamily: 'sans', align: 'center', lines: 3, karaoke: true,
      titleColor: '#475569', iconColor: '#1f2430',
      menuColor: '#f5f1e8', menuOpacity: 85,
    },
  },
  {
    name: 'game',
    values: {
      opacity: 20, glassColor: '#000000', fontPx: 22, fontColor: '#ffffff', fillColor: '#4ade80',
      subColor: '#d1d5db', subFontPx: 12, romColor: '#d1d5db', romFontPx: 12, outline: true,
      outlineColor: '#000000', outlineWidth: 3, fontFamily: 'mono', align: 'left', lines: 1, karaoke: true,
      titleColor: '#d1d5db', iconColor: '#ffffff',
      menuColor: '#000000', menuOpacity: 40,
    },
  },
];

/** The look the overlay has right now (what "Save look" stores). */
export function currentLook(s: Settings): LookValues {
  const out: LookValues = {};
  for (const k of LOOK_KEYS) out[k] = s[k];
  return out;
}

/** True when every value of the preset is what the overlay is using now. */
export function sameLook(s: Settings, values: LookValues): boolean {
  return Object.entries(values).every(([k, v]) => {
    const cur = (s as unknown as LookValues)[k];
    return typeof v === 'string' && typeof cur === 'string' ? v.toLowerCase() === cur.toLowerCase() : v === cur;
  });
}
