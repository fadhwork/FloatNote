import type { Settings } from './api';

const FONTS: Record<Settings['fontFamily'], string> = {
  sans: "'Segoe UI Variable Display','Segoe UI',system-ui,sans-serif",
  display: "Bahnschrift,'Segoe UI Semibold','Segoe UI',system-ui,sans-serif",
  serif: "'Georgia','Cambria','Times New Roman',serif",
  mono: "'Cascadia Code','Cascadia Mono',Consolas,monospace",
};

/** Text outline built from shadows, so it stays crisp on any background. */
function outlineShadow(width: number, color: string): string {
  const soft = '0 2px 12px rgba(0,0,0,.35)';
  if (width <= 0) return soft;
  const parts: string[] = [];
  for (const r of [width, width / 2]) {
    for (let a = 0; a < 360; a += 30) {
      const x = (Math.cos((a * Math.PI) / 180) * r).toFixed(2);
      const y = (Math.sin((a * Math.PI) / 180) * r).toFixed(2);
      parts.push(`${x}px ${y}px 0 ${color}`);
    }
  }
  parts.push(soft);
  return parts.join(',');
}

export function applyTheme(s: Settings): void {
  const root = document.documentElement.style;
  const color = s.glassColor.replace(/^#/, '');
  const hex = color.length === 3 || color.length === 4
    ? [...color.slice(0, 3)].map((v) => `${v}${v}`).join('')
    : color.slice(0, 6);
  const rgb = hex.match(/.{2}/g)?.map((v) => Number.parseInt(v, 16));
  root.setProperty('--panel-alpha', String(s.opacity / 100));
  if (rgb?.length === 3 && rgb.every(Number.isFinite)) {
    root.setProperty('--glass-rgb', rgb.join(', '));
  }
  root.setProperty('--font-px', `${s.fontPx}px`);
  root.setProperty('--font-color', s.fontColor);
  root.setProperty('--fill-color', s.fillColor);
  root.setProperty('--sub-color', s.subColor);
  root.setProperty('--sub-font-px', `${s.subFontPx}px`);
  root.setProperty('--rom-color', s.romColor);
  root.setProperty('--rom-font-px', `${s.romFontPx}px`);
  root.setProperty('--font-family', FONTS[s.fontFamily] ?? FONTS.sans);
  root.setProperty('--align', s.align);
  root.setProperty('--outline', s.outline ? outlineShadow(s.outlineWidth, s.outlineColor) : 'none');
}
