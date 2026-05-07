import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    svelte(),
    tailwindcss({
      content: ['src/**/*.{html,js,svelte,ts}'],
    }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/target/**'],
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
