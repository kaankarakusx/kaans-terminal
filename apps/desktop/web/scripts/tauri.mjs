// Tauri CLI'ı web/ klasöründen çalıştırır; src-tauri kardeş klasörde olduğu için yolu biz veririz.
// Platformdan bağımsız (Windows'ta `VAR=x cmd` sözdizimi çalışmaz).
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const web = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const r = spawnSync('tauri', process.argv.slice(2), {
  stdio: 'inherit',
  shell: true,
  cwd: web,
  env: { ...process.env, TAURI_APP_PATH: resolve(web, '../src-tauri'), TAURI_FRONTEND_PATH: web },
})
process.exit(r.status ?? 1)
