import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';

const page = (name: string) => fileURLToPath(new URL(name, import.meta.url));

export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: 'localhost' },
  build: {
    target: 'chrome105', // WebView2 is Chromium
    rollupOptions: {
      input: { main: page('index.html'), settings: page('settings.html') },
    },
  },
});
