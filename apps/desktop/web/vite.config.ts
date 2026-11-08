import { svelte } from '@sveltejs/vite-plugin-svelte'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

/** packages/shared: uygulama ve sunucunun ortak kuralları */
const shared = fileURLToPath(new URL('../../../packages/shared/src', import.meta.url))

// Tauri devUrl ile aynı port; port doluysa başka porta kaymasın.
export default defineConfig({
  plugins: [svelte()],
  resolve: { alias: { '@kaans/shared': shared } },
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] }, fs: { allow: ['.', shared] } },
  build: { target: 'safari16', outDir: 'dist' },
})
