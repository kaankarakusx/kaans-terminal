#!/usr/bin/env node
// Sürüm paketi (Mac): imzalı güncelleme dosyaları + latest.json. YAYINLAMAZ; GitHub'a yükleme komutunu yazar.
//   node apps/desktop/src-tauri/scripts/release.mjs            (repo kökünden ya da herhangi bir yerden)
//
// Önce sürümü artır: apps/desktop/src-tauri/tauri.conf.json "version" + kök Cargo.toml [workspace.package] version
// (ikisi aynı olmalı). Sürüm notları: apps/desktop/src-tauri/release-notes.md (yoksa kısa varsayılan metin).
//
// İmza anahtarı (Ed25519, tauri signer): ~/.tauri/kaans-terminal.key; şifresi macOS Anahtar Zinciri'nde
// (servis "kaans-terminal-updater"). Anahtar ya da şifre ekrana/dosyaya yazılmaz, yalnızca bu sürecin ortamına girer.
// Anahtar kaybolursa yüklü sürümler yeni güncellemeyi kabul etmez: yedekle.
import { execFileSync, spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const REPO = 'kaankarakusx/kaans-terminal'
const here = dirname(fileURLToPath(import.meta.url))
const tauriDir = resolve(here, '..')
const webDir = resolve(tauriDir, '../web')
const root = resolve(tauriDir, '../../..')

const conf = JSON.parse(readFileSync(join(tauriDir, 'tauri.conf.json'), 'utf8'))
const version = conf.version
const cargoVersion = /\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s.exec(readFileSync(join(root, 'Cargo.toml'), 'utf8'))?.[1]
if (cargoVersion && cargoVersion !== version) {
  console.error(`sürüm uyuşmuyor: tauri.conf.json ${version}, Cargo.toml ${cargoVersion}`)
  process.exit(1)
}

const keyPath = join(homedir(), '.tauri/kaans-terminal.key')
if (!existsSync(keyPath)) {
  console.error(`imza anahtarı yok: ${keyPath}`)
  process.exit(1)
}
let password
try {
  password = execFileSync('security', ['find-generic-password', '-s', 'kaans-terminal-updater', '-w'], {
    stdio: ['ignore', 'pipe', 'ignore'],
  })
    .toString()
    .trim()
} catch {
  console.error('Anahtar Zinciri\'nde "kaans-terminal-updater" şifresi bulunamadı')
  process.exit(1)
}

console.log(`Kaan's Terminal ${version} derleniyor (imzalı güncelleme dosyalarıyla)…`)
const build = spawnSync(
  'npm',
  ['run', 'tauri', '--', 'build', '--config', join(tauriDir, 'tauri.release.conf.json')],
  {
    cwd: webDir,
    stdio: 'inherit',
    env: {
      ...process.env,
      TAURI_SIGNING_PRIVATE_KEY: readFileSync(keyPath, 'utf8'),
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password,
    },
  },
)
if (build.status !== 0) process.exit(build.status ?? 1)

// Mac güncelleme dosyası: bundle/macos/<ad>.app.tar.gz + .sig
const macDir = join(root, 'target/release/bundle/macos')
const archive = readdirSync(macDir).find((f) => f.endsWith('.app.tar.gz'))
if (!archive || !existsSync(join(macDir, `${archive}.sig`))) {
  console.error(`güncelleme dosyası ya da imzası yok: ${macDir}`)
  process.exit(1)
}
const arch = process.arch === 'arm64' ? 'aarch64' : 'x86_64'
// GitHub dosya adındaki boşluk/kesme işaretini değiştirir: URL tutsun diye güvenli adla kopya
const assetName = `Kaans-Terminal_${version}_${arch}.app.tar.gz`
copyFileSync(join(macDir, archive), join(macDir, assetName))
const notesPath = join(tauriDir, 'release-notes.md')
const notes = existsSync(notesPath) ? readFileSync(notesPath, 'utf8').trim() : 'Bug fixes and improvements.'
const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    [`darwin-${arch}`]: {
      signature: readFileSync(join(macDir, `${archive}.sig`), 'utf8').trim(),
      url: `https://github.com/${REPO}/releases/download/v${version}/${assetName}`,
    },
  },
}
const latestPath = join(macDir, 'latest.json')
writeFileSync(latestPath, JSON.stringify(latest, null, 2) + '\n')

const dmgDir = join(root, 'target/release/bundle/dmg')
const dmg = existsSync(dmgDir) ? readdirSync(dmgDir).find((f) => f.endsWith('.dmg')) : null
console.log(`
Hazır (yayınlanmadı):
  ${join(macDir, assetName)}
  ${latestPath}${dmg ? `\n  ${join(dmgDir, dmg)}` : ''}

GitHub'a yüklemek için (gh CLI ile; yayınlamak senin kararın):
  gh release create v${version} --repo ${REPO} --title "v${version}" --notes-file "${notesPath}" \\
    "${join(macDir, assetName)}" "${latestPath}"${dmg ? ` "${join(dmgDir, dmg)}"` : ''}

Uygulamalar https://github.com/${REPO}/releases/latest/download/latest.json adresini okur: en son yayın "latest"
olmalı (taslak/ön sürüm değil). Windows paketi ayrı derlenir (Windows'ta ya da CI'da) ve latest.json'a
"windows-x86_64" girdisi eklenir.`)
