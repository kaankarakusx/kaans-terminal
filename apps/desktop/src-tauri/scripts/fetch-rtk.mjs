// rtk-oss'u (Apache-2.0) sabit sürüm ve SHA-256 doğrulamasıyla indirip pakete koyar (OZELLIKLER.md §7, MIMARI.md §8).
// Varsayılan "rtk" sürümü BUSL-1.1 bir motoru içine bağladığı için yalnızca saf Apache-2.0 olan rtk-oss kullanılır.
// Çıktı: resources/bin/<hedef>/rtk[.exe] ve resources/licenses/rtk-LICENSE. İkili dosyalar git'e girmez.
//   node scripts/fetch-rtk.mjs           (eksik olanları indirir)
//   node scripts/fetch-rtk.mjs --check   (yalnızca var olanları doğrular)
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync, chmodSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const VERSION = 'v1.1.0'
const BASE = `https://github.com/rtk-ai/rtk/releases/download/${VERSION}`
// checksums.txt (v1.1.0) içinden; sürüm değişince birlikte güncellenir.
const ASSETS = [
  { target: 'aarch64-apple-darwin', file: 'rtk-oss-aarch64-apple-darwin.tar.gz', bin: 'rtk', sha256: '1623590dd8d65374f61589c599c764c08ba9ce993ebe6b601312a89a3c874558' },
  { target: 'x86_64-apple-darwin', file: 'rtk-oss-x86_64-apple-darwin.tar.gz', bin: 'rtk', sha256: '559156238dd3e148c84c88c3a62a7594b874f68793208994e1b02e77a9d9abf9' },
  { target: 'x86_64-pc-windows-msvc', file: 'rtk-oss-x86_64-pc-windows-msvc.zip', bin: 'rtk.exe', sha256: '9b076379be9d86011ca3f3a0264d0be4851da5535453a47f67fa46b4e893fbb2' },
]

const root = join(dirname(fileURLToPath(import.meta.url)), '..', 'resources')
const checkOnly = process.argv.includes('--check')
const sha = (buf) => createHash('sha256').update(buf).digest('hex')

async function download(url) {
  const res = await fetch(url, { redirect: 'follow' })
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`)
  return Buffer.from(await res.arrayBuffer())
}

const marker = (dir) => join(dir, '.source')

for (const a of ASSETS) {
  const dir = join(root, 'bin', a.target)
  const out = join(dir, a.bin)
  const source = `${a.file} ${a.sha256}`
  if (existsSync(out) && existsSync(marker(dir)) && readFileSync(marker(dir), 'utf8').trim() === source) {
    console.log(`ok      ${a.target}`)
    continue
  }
  if (checkOnly) {
    console.error(`missing ${a.target} (node scripts/fetch-rtk.mjs)`)
    process.exitCode = 1
    continue
  }
  const buf = await download(`${BASE}/${a.file}`)
  const got = sha(buf)
  if (got !== a.sha256) throw new Error(`${a.file}: SHA-256 uyuşmuyor (${got}); pakete konmadı`)
  // İndirilen arşiv kendi boş klasöründe açılır; içinden hiçbir şey çalıştırılmaz.
  const work = mkdtempSync(join(tmpdir(), 'rtk-'))
  try {
    const archive = join(work, a.file)
    writeFileSync(archive, buf)
    const unpack = join(work, 'x')
    mkdirSync(unpack)
    if (a.file.endsWith('.zip')) execFileSync('unzip', ['-q', archive, '-d', unpack])
    else execFileSync('tar', ['-xzf', archive, '-C', unpack])
    const found = execFileSync('find', [unpack, '-type', 'f', '-name', a.bin]).toString().trim().split('\n')[0]
    if (!found) throw new Error(`${a.file}: arşivde ${a.bin} yok`)
    mkdirSync(dir, { recursive: true })
    renameSync(found, out)
    if (!a.bin.endsWith('.exe')) chmodSync(out, 0o755)
    writeFileSync(marker(dir), source + '\n')
    console.log(`fetched ${a.target} (${(buf.length / 1e6).toFixed(1)} MB, sha256 ok)`)
  } finally {
    rmSync(work, { recursive: true, force: true })
  }
}

const lic = join(root, 'licenses', 'rtk-LICENSE')
if (!existsSync(lic) && !checkOnly) {
  mkdirSync(dirname(lic), { recursive: true })
  const text = await download(`https://raw.githubusercontent.com/rtk-ai/rtk/${VERSION}/LICENSE`)
  writeFileSync(lic, `rtk-oss ${VERSION} — https://github.com/rtk-ai/rtk (Apache-2.0)\n\n${text}`)
  console.log('fetched rtk LICENSE')
}
