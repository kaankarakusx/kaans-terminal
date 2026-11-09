# Kaan's Terminal

Birçok yapay zekâ kodlama oturumunu (Claude Code, Codex, Antigravity…) aynı anda yürüten geliştiriciler için
masaüstü HUD + terminal uygulaması. Tauri 2 (Rust) + Svelte 5 (TypeScript). Hedef: macOS 13+ ve Windows 10/11.

Kodda ad `APP_NAME` sabitinden gelir (`apps/desktop/src-tauri/src/lib.rs`).

## Klasör yapısı
```
Cargo.toml                    # Cargo workspace (resolver 3, edition 2024)
apps/desktop/
  src-tauri/                  # Rust: Tauri uygulaması
    src/lib.rs                # Builder, komutlar, APP_NAME
    src/engine/               # salt okunur okuyucu
    src/host/                 # kalıcı terminal oturum sunucusu (protokol, sunucu, istemci)
    src/term.rs               # terminal komutları ve pencereleri
    src/procs.rs              # süreç tablosu (libproc / ToolHelp)
    src/power.rs              # pil / düşük güç
    src/gesture.rs            # Mac dört parmak tıklaması → kısayol çarkı (NSEvent yerel izleyici)
    src/integrations.rs       # harici ayarlara dokunan entegrasyonlar (izinli, yedekli, geri alınabilir)
    src/git.rs                # Source Control: kullanıcının git'i (status/stage/commit/push/sync/log)
    resources/pricing.json    # api-eq fiyat tablosu (gömülü; uygulama veri klasöründeki pricing.json ezer)
    fixtures/claude/          # engine testleri için sahte $CLAUDE_DIR
    examples/engine_dump.rs   # gerçek veride engine özeti
    tauri.conf.json           # identifier: com.kaankarakus.kaans-terminal
    capabilities/             # Tauri izinleri (en az yetki)
  web/                        # Svelte 5 + Vite 8 + TS (dashboard)
    src/themes/               # tokens.css, default.json, palettes/*.json (12 tema), index.ts (sıra), store.svelte.ts
                              #   (seçim/önizleme/yazı tipi/boyut → localStorage), color.ts (OKLCH), derive.ts, apply.ts
    src/lib/                  # bileşenler, engine köprüsü (engine.svelte.ts), tipler (types.ts)
      panels/                 # research.feed, files.heat, tool.mix, workers, spend.ledger
      orbit/                  # three.js sahnesi (scene.ts, Svelte'ten bağımsız) + OrbitMap.svelte + palette.ts
      layout.svelte.ts        # panel düzeni (alanlar, ağırlıklar, gizli, temizlenen) → localStorage
      notify.svelte.ts        # needs you / same file bildirimleri, ses, sistem bildirimi
      power.svelte.ts         # ön plan/arka plan, pil, kullanıcı hareketsizliği; <html class="behind">
      demo.ts                 # yalnızca dev: Tauri olmadan tarayıcıda sahte veri
      git.svelte.ts           # Source Control durumu (yoklama, işlemler, demo arka ucu); SourceControl.svelte çekmece
      gitGraph.ts             # commit grafiği şerit yerleşimi (test: gitGraph.test.ts)
      shortcuts.svelte.ts     # kısayol kayıt defteri (e.code ile), SHORTCUTS ekranı ve çark bunu kullanır
      terminals.ts            # openTerminal / openChat / newTerminal
    src/term/                 # sohbet/terminal penceresi: TermApp, model (sekme+bölme ağacı), TerminalPane (xterm)
    scripts/tauri.mjs         # Tauri CLI sarmalayıcı (src-tauri yolunu verir)
```
`packages/shared/src/` — uygulama ve sunucunun ortak kuralları (TS): `xp.ts` (XP, level eğrisi, 7 rütbe, seri,
heartbeat kredisi). Web'de `@kaans/shared/*` takma adıyla (vite.config + tsconfig paths); testleri web'in vitest'iyle
koşar (`src/lib/sharedXp.test.ts`).
`server/` — sosyal sunucu (Fastify 5 + Postgres + ileride WS), Node ≥22.18 TypeScript'i doğrudan çalıştırır (derleme
yok, importlarda `.ts` uzantısı).

## Komutlar
```sh
# web/ içinden
cd apps/desktop/web
npm install
npm run tauri -- dev          # uygulamayı geliştirme modunda aç (Vite :1420 + Rust)
npm run tauri -- build        # paket üret
npm run check                 # svelte-check + tsc (uygulama, vite config, testler)
npm test                      # vitest (tema türetme vb.)
npm run build                 # yalnızca web derlemesi
npm run dev                   # yalnızca Vite; tarayıcıda http://localhost:1420 → demo verisiyle arayüz
                              #   terminal penceresi demosu: http://localhost:1420/#term=demo-win

# repo kökünden
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt

# server/ içinden
npm run dev                   # yerel sunucu :8787 (DATABASE_URL yoksa ./data/dev-db içinde PGlite)
npm test                      # vitest, bellekte PGlite (gerçek Postgres gerekmez)
npm run check                 # tsc --noEmit
```
Tauri CLI'ı doğrudan `npx tauri` ile çalıştırma; `web/` içinden `src-tauri`'yi bulamaz. Her zaman `npm run tauri`.

## Sürümler
- Tauri **2.x** (kararlı). crates.io'da 3.0 alfa var; kullanma.
- Svelte 5: runes kullan (`$state`, `$derived`, `$props`, `$effect`). `export let` ve store'lar yerine runes.
- Rust edition 2024. Yeni bağımlılık eklerken versiyonu workspace'e değil crate'e yaz, gereksiz özellik açma.

## Değişmez kurallar
1. **Engine salt okunurdur.** `~/.claude`, `~/.codex`, `~/.gemini` altında hiçbir dosyaya yazılmaz.
2. **`~/.claude/sessions/` içindeki `.key` dosyaları asla okunmaz.** Yalnızca `^\d+\.json$` eşleşen dosyalar.
3. `$CLAUDE_DIR` = ayarlarda seçili Claude profili, yoksa `CLAUDE_CONFIG_DIR` ortam değişkeni, o da yoksa `~/.claude`
   (`paths::claude_dir()`). Yolu elle `~/.claude` diye sabitleme.
4. **Oturum verisi cihazdan çıkmaz** (prompt, kod, transcript, dosya adı, token). Sunucuya yalnızca sosyal veri gider.
5. **Arayüze giden her metin `redact()`'ten geçer**. Yeni bir metin alanı eklerken unutma.
6. **Harici araçların ayarlarına izinsiz dokunulmaz**: önce kullanıcıya sor, yedek al, geri alınabilir yap.
7. Süreç bilgisi kernel'den okunur (Mac: `libproc`/`sysctl`, Windows: `NtQuerySystemInformation`). Döngü içinde
   `ps` gibi harici komut çalıştırma.
8. Fiyat tablosu kodda sabit değil, JSON dosyasında.
9. CSP sıkı kalır (`default-src 'self'`); uzak içerik/CDN yüklenmez. Her şey paketlenir.
10. Analitik/takip kütüphanesi eklenmez.

## Arayüz kuralları
- Arayüzdeki bütün metinler **İngilizce**. Panel adları kod gibi: `orbit.map`, `session.log`, `your.call`…
- Her şey monospace. Yalnızca koyu temalar.
- **Renkler yalnızca `tokens.css` değişkenlerinden** gelir; bileşenlerde ham hex yazma. Saydamlık için
  `rgba(var(--accent-rgb), .3)`.
- Durum renkleri: working `--accent`, your turn `--cyan`, needs you `--warn`, subagent `--violet`, ended `--ended`.
  Durum yalnızca renkle anlatılmaz, yanında yazı/ikon olur.
- `prefers-reduced-motion` desteklenir; animasyonlar uygulama arkadayken durur.
- Tema: `themes/default.json` + `themes/palettes/*.json` → `deriveTheme()` →
  `applyTokens()`; `main.ts` bileşenler bağlanmadan önce `themes.init()` çağırır. Seçim/önizleme/yazı tipi/boyut
  localStorage'da; diğer pencereler `storage` olayıyla anında güncellenir. 16 ANSI rengi `--ansi-0…15` token'ı
  (xterm `currentAnsi()`). Yeni tema: JSON + `index.ts` ORDER + `LICENSES.txt`; parlak ANSI renkleri gri olan
  temada anlam renklerini `ui` ile ver (Solarized).
- İmza tema `palettes/signature.json` (`"signature": true`) → `<html class="sig">` → `themes/signature.css`
  (çizgili etiketler, taramalı başlıklar, köşe süsleri, ızgaralı üst bar, filigran, imleç). Görsel katman yalnızca
  `html.sig` altında yazılır. Etiket/logo yazı tipi Space Grotesk 700 (`@fontsource/space-grotesk`, sürüm sabit,
  OFL lisansı `src-tauri/resources/licenses/SpaceGrotesk-OFL.txt`); terminal ve kod monospace kalır.
- Lisans: MIT (`LICENSE`). Paketlenen üçüncü taraf lisansları `src-tauri/resources/licenses/`. `window.__theme(theme)` Rust → sayfa köprüsüdür; değişince `themechange` olayı yayılır.
- Kısayollar: ⌘B / Ctrl+B sohbet listesini (`sessions` paneli) açıp kapatır. ⌃⇧G Source Control.
- Ana ızgaranın ilk sütunu activity bar (`ActivityBar.svelte`, 44px): sohbetler, Source Control (rozetli), temalar,
  ayarlar. Tema sayfası ve Source Control (`ui.scm`) sol sütunda `PanelArea left`'in yerine açılır (VS Code gibi).
- Sohbet listesi yalnızca şu an açık olanları gösterir: bitmiş sohbetler gizli; uygulamanın her açık terminali
  `TermCard.svelte` kutusuyla (RUNNING + komut ya da READY + "shell ready"; kabuk adı gösterilmez; tıkla = terminal
  penceresi). Aynı penceredeki (bölme/sekme) terminaller tek kart (`windowFor`, rozet "N panes"; kırmızı ışık hepsini
  bitirir). Bölünmüş pencerede her bölmenin üstünde başlık şeridi (numara, ad, ✕ = bölmeyi kapat, oturum yaşar). orbit.map'te bitmiş sohbet yok, terminal rozetinde komut ya da proje adı.
- Oturum kartı: üç ışık (kırmızı = oturumu bitir, yalnızca uygulamanın terminalindeki sohbetler, onaylı; sarı =
  katla; yeşil = aç), proje, `window` rozeti, durum sayacı, başlık, klasör, "thinking… last <araç>", ctx, sayılar.
- Terminal penceresi düzeni: Mac'te sistem başlık çubuğu yok (`TitleBarStyle::Overlay` + `hidden_title`), trafik
  ışıkları bizim başlık satırımızda (`data-tauri-drag-region`, soldan 82px boşluk). Başlık: proje · sohbet · durum
  rozeti (WORKING mm:ss = turun süresi, `turnStarted`). Bilgi şeridi: ctx çubuğu, model, effort, $, araçlar.
  Alt şerit: sohbetin son araç çağrıları. Tek sekmede sekme çubuğu gizli. Svelte'te değişkene `state` adı verme
  (`$state` rünüyle çakışır).
- Terminal pencereleri saydamdır (`macos-private-api` + `.transparent(true)`), zemin `rgba(var(--term-bg-rgb), .96)`.
  xterm'in **WebGL çizicisi saydam zemini desteklemez** (siyah çizer): saydamken DOM çizici kullanılır
  (`TerminalPane.svelte` → `TRANSPARENT`). `:root { color-scheme: dark }` olmadan WebKit kaydırma çubuğu beyaz çıkar.
- Tema sayfası `ThemesPage.svelte` (`ui.themes`, activity bar paleti, ⌘⇧,): ana alanı kaplar (`.over`), kart =
  uygulamanın o temadaki maketi; üzerine gelince önizleme, tıklayınca seçim.
- Ayarlar sayfası `SettingsPage.svelte` (`ui.settings`; ana alanı kaplar, tema/sıralama sayfası gibi): INTERFACE,
  INTEGRATIONS, SHORTCUTS, ABOUT sekmeleri (`ui.openSettings(tab)`); ilk açılışta Entegrasyonlar bir kez gösterilir.
  Sol sütun görünümü (`ui.side`) açılınca sayfalar kapanır (yoksa sayfanın arkasında kalır).
- Yeni panel: `layout.svelte.ts` (`PanelId`, varsayılan alan) + `panels.ts` kaydı + `<Panel id=…>`. Temizlenebilir
  panel `layout.clearedAt(id)` ile süzer; toplam sayan panel `onclear` ile taban değer verir.
- orbit.map temadan bağımsızdır (`orbit/palette.ts`). Sahne `setPower()` ile kare hızını/çözünürlüğü sınırlar.
  Düzen: her şey tek yatay düzlemde (kamera hafif yukarıdan). Ortada HUD çekirdeği (`orbit/hud.ts` `CoreHud`:
  tools/min + halkalar cache/5h/week/load, `setCore`); oturumlar `RINGS` halkalarında eşit aralıklı dönen rozetler
  (`sessionBadge`: proje + durum); istasyonlar dış halkada (`stationTexture` simge, HTML etiket `files.read 248`,
  altında son hedef; `setStations`). Terminaller (`vehicles.ts`, API adı hâlâ "car") araba değil rozet
  (`terminalBadge`: iç içe halkalar + altıgen, ad + RUNNING/READY/LEFTOVER); çalışan terminal kendi halkasında
  (oturumlarla istasyonlar arası) döner, boştaki sol alta park eder. Alt satırda durum renkleri + ipuçları, sağ altta
  minimap. Çalışan uygulamalar (`Snapshot.apps`) istasyonların dışındaki şeritlerde gezen arabalar (CPU arttıkça
  hızlanır; etiket `npm run dev` / `proje · 12% cpu`; tık = bağlı terminal ya da iTerm/Terminal penceresi, ikisi de
  yoksa kapatmayı sorar). İçinde uygulama çalışan terminal yalnızca araba: listede kartı ve haritada rozeti yok;
  uygulama olan kalan süreç (leftover) da yalnızca araba. `danger_kill` kaçak süreçlere ve terminalsiz uygulamalara
  (alt süreçleriyle) izin verir; başka pid kabul etmez.
- Arayüzü görmek için: `npm run dev` + tarayıcı (demo modu). Ekranı göremiyorsan Chrome ile ekran görüntüsü al.

## Engine (`src-tauri/src/engine/`)
| Modül | Görev |
|---|---|
| `mod.rs` | `Engine::tick()`: kayıtlar → kartlar, transcript keşfi, tail, `Snapshot` + yeni `EngineEvent`'ler |
| `registry.rs` | `sessions/<pid>.json` (mtime önbellekli; `.key` filtresi `paths::is_registry_file`) |
| `transcript.rs` | `Tail` (bayt konumundan devam, yarım satır saklama) ve `parse_line` |
| `status.rs` | `status_of` / `subagent_running` (saf fonksiyonlar) |
| `tools.rs` | araç adı → kategori, hedef, dosya yolu |
| `redact.rs` | `redact()` ve `clean()` (maskele + tek satır + kısalt) |
| `proc.rs` | süreç canlılığı + başlangıç zamanı (Mac `proc_pidinfo`, Windows `GetProcessTimes`); `procStart` ile pid yeniden kullanımı ayırt edilir |
| `pricing.rs` | `resources/pricing.json`: en uzun önek eşleşmesi, önbellek 5 dk / 1 sa yazma, uzun bağlam çarpanı, bağlam penceresi |
| `settings.rs` | `/autocompact` penceresi: env → proje local → proje → kullanıcı `settings.json` |
| `ledger.rs` | harcama defteri: geçmiş günler bir kez okunup dondurulur (`<app_data>/ledger.json`), `PastTotals` |
| `codex.rs` | Codex rollout'ları → ortak `Item` akışı (tur, model, bağlam, limitler; token sayılır, fiyatlanmaz) |
| `apps.rs` | çalışan uygulama komutları (`npm run dev`, `vite`, `cargo run`, `uvicorn`…): zincirde en dıştaki, CPU alt süreçlerle toplam; npm'in yeniden yazdığı süreç adı/başlığı (`npm run tauri de`, argv = tek metin + boşluklar) da tanınır. Gerçek veride: `cargo run --example apps_dump` |
| `agents.rs` | süreç tablosundan diğer yapay zekâ araçları (Gemini, Antigravity, Aider…); node betikleri argv'den |
| `tests.rs` | geçici klasörde sahte `~/.claude` ve `fixtures/claude/` ile uçtan uca testler |

`src/power.rs`: pil / düşük güç (Mac IOKit + `NSProcessInfo`, Windows `GetSystemPowerStatus`).

İş parçacıkları (`lib.rs`): `engine` her saniye tur atar; `ledger` açılışta ve dakikada bir dünü dondurur, geçmiş
toplamları kanalla engine'e yollar. İlk açılışta ~1 GB transcript debug'da ~6 sn, release'de ~0,6 sn sürer.

Bilinen veri ayrıntıları:
- Bir assistant mesajı her içerik bloğu için ayrı satırdır ve aynı `message.id` + `usage` tekrar eder → kullanım
  `usage_seen` ile mesaj başına bir kez sayılır.
- Fork'lar ebeveynin satırlarını aynı `uuid` ile kopyalar → `seen_uuids`.
- Alt ajanlar: `projects/<enc-cwd>/<sessionId>/subagents/agent-*.jsonl` + `.meta.json` (`agentType`, `description`,
  `finished`).
- Sayaçlar "bugün" (yerel gün) içindir; gün değişince engine sıfırdan kurulur.
- Durum: canlı kaydı bir kez görülmüş oturumun kaydı kaybolursa ya da süreci ölürse hemen `ended`
  (`StatusInput::record_gone`, `Engine::seen_live`). "Kayıt yok ama son 90 sn'de yazdı = çalışıyor" kuralı yalnızca
  kaydı hiç görülmemiş oturumlar (eski sürüm, SDK) için.
- `procStart` UTC metnidir (`"Sun Nov  8 10:24:45 2026"`); kernel'in başlangıç zamanıyla ±2 sn karşılaştırılır.
- Maliyet, mesaj başına birleştirilmiş kullanımın farkından hesaplanır; uzun bağlam eşiği istemin boyutuna bakar.
  Fiyat tablosunda olmayan model "fiyatsız"dır (`unpriced`), arayüzde maliyetin yanında `+` görünür.
- Fiyatları güncellerken kaynağı yaz (`version`), `pricing.rs` testlerini güncelle.
- Same file: son 20 dk'da aynı yola yazan **farklı kartlar** (alt ajan kendi kartına sayılır).
- Rust'ta boş `f64` toplamı `-0.0` döner; arayüze giden toplamlara `+ 0.0` ekle.
- macOS dosya sistemi büyük-küçük harf ayırmaz: `foo.svelte.ts` ile `Foo.svelte` aynı klasörde olmasın (Vite
  `./foo.svelte` importunu bileşene çözer).

Gerçek veride hızlı deneme (yalnızca sayıları basar): `cargo run --release --example engine_dump`

Codex: `with_codex(codex_dir())` yalnızca uygulamada açılır (testler gerçek `~/.codex`'e bağlanmasın). Kart
kimliği `codex:<rollout dosya adı>`; durum = aynı klasörde çalışan `codex` süreci + tur açık/kapalı;
`request_user_input` → "needs you". Gerçek veride geçmiş bir gün: `cargo run --example codex_dump -- 2026-10-10`.
Antigravity için yerel veri yok: şimdilik yalnızca süreçten (çalışıyor/sıra sende); kayıt biçimi görülünce
okuyucusu yazılacak (tahminle yazma).

## Claude profilleri (`src/profiles.rs`, `lib/profiles.svelte.ts`, Ayarlar → PROFILES)
- Profil = ayrı `CLAUDE_CONFIG_DIR` klasörü (ör. `alias claude-work='CLAUDE_CONFIG_DIR="$HOME/.claude-work" claude'`).
  Algılama salt okunur: varsayılan klasör + kabuk dosyalarındaki takma adlar + `~/.claude*` klasörleri (`projects/` ya
  da `settings.json` olan). Seçim `<app_data>/profile.json`; `paths::set_active` + `paths::generation()`.
- Yalnızca seçili profil gösterilir: profil değişince engine ve harcama defteri o klasörle sıfırdan kurulur, sayfa
  `profile://changed` ile eski olayları siler. Codex etkilenmez. Terminallere dokunulmaz.
- Profile özel uygulama dosyaları `profiles::file_name(ad, id)`: `~/.claude` için eski adlar (`ledger.json`,
  `claude-limits.json`), diğerleri `-<id>` sonekli (`ledger-claude-work.json`). `--statusline` kipi profilini Claude'un
  aktardığı `CLAUDE_CONFIG_DIR`'den çıkarır. Limit/tema entegrasyonu seçili profilin `settings.json`'una yazar;
  rtk/caveman varsayılan profilde kalır.

## Entegrasyonlar (`src/integrations.rs`)
- Harici ayarlar YALNIZCA `integration_set` komutuyla (kullanıcı Entegrasyonlar ekranında açınca) değişir.
- Yazmadan önce `<app_data>/backups/<dosya>.<zaman>` yedeği; JSON okunamıyorsa dokunulmaz; `serde_json`
  `preserve_order` açık (kullanıcının anahtar sırası bozulmasın).
- Claude limitleri: statusLine = `'<exe>' --statusline` (aynı ikili dosya). Yalnızca `rate_limits` →
  `<app_data>/claude-limits.json`; kullanıcının önceki statusLine'ı `statusline-next.json`'da, aynı stdin ile çalışır.
- Claude teması: `theme` = `dark-ansi` (terminalin 16 rengi); önceki değer `claude-theme-prev.json`'da. Kullanıcı arada
  temayı değiştirdiyse kapatırken onun seçimi kalır. Codex/Antigravity tema anahtarı belgelenmedi: tahminle ekleme.
- Testler her zaman geçici klasörde; gerçek `~/.claude/settings.json`'a geliştirirken de elle dokunma.

## XP ve rütbeler (`src/activity.rs`, `lib/xp.svelte.ts`)
- Rust yalnızca **günlük aktif saniye** tutar (`<app_data>/activity.json`): dakikada bir örnek; aktif = sistem genelinde
  son 5 dk'da klavye/fare (Mac `CGEventSourceSecondsSinceLastEventType`, Windows `GetLastInputInfo`) **ve** çalışma
  sinyali (son 5 dk'da sohbet etkinliği ya da çalışan terminal komutu). Kredi sunucu kuralıyla aynı (≤90 sn, boşluk
  >3 dk ise 60 sn). `activity_days` komutu + `xp://activity` olayı.
- XP/level/rütbe sayfada `@kaans/shared/xp` ile hesaplanır (sunucu gelince aynı kod orada). Kuralı değiştirirken
  yalnızca `packages/shared/src/xp.ts` + testleri; Rust'taki `activity::credit` heartbeat kuralının kopyasıdır.
- Profil ve sıralama `LeaderboardPage.svelte` (`ui.board`; activity bar'daki kupa). Ana alanı
  kaplayan sayfalar `ui.page('themes' | 'board' | 'settings')` ile açılır (aynı anda biri). Arkadaş/genel liste sunucuyla gelir.
- Rütbe renkleri temadan bağımsız (`RANKS`); rozetler `RankBadge.svelte` (kendi çizimimiz). Kutlama `Celebration.svelte`
  yalnızca uygulama öndeyken; ses `notifier.sound` açıksa. Demo'da `__xp.preview('rank')`.

## Source Control (`src/git.rs`, `lib/SourceControl.svelte`)
- Kullanıcının kendi `git`'i çalışır (ayarları, kimlik yardımcısı, hook'ları geçerli); `GIT_TERMINAL_PROMPT=0`,
  süre sınırı (yerel 30 sn, ağ 120 sn). Mac'te `/usr/bin/git` yalnızca Xcode araçları kuruluysa (yoksa kurulum
  penceresi açan kabuktur); önce Homebrew kopyaları.
- Repolar = sohbetlerin + terminallerin klasörlerinin kökleri (ev klasörü hariç). Komutlar `checked_root` (mutlak +
  `.git` var) ve `checked_paths` (köke göre, `..` yok) ile korunur.
- Hata metinleri `tidy_error`: URL'deki kullanıcı:parola gizlenir + `redact()`. Commit konuları `redact()`'ten geçer.
  Dosya yolları işlemlerde kimlik olduğu için maskelenmez.
- Yoklama: panel açıkken 4 sn, kapalıyken (rozet) 20 sn, uygulama arkadayken yok. Discard yalnızca izlenen dosyalar
  (izlenmeyen dosya silinmez) ve onay ister. ✦ = bugün bir sohbetin yazdığı dosya (`git_edited`).
- Gerçek git testi geçici klasörde (`git::tests::real_git_round_trip`); git yoksa atlanır.
- Kod görüntüleyici `lib/code/CodeView.svelte` (`ui.code`; SCM'de dosyaya tıkla): `git_diff` eski/yeni metin + patch,
  `git_file`, `git_blame`. Dosya/patch içeriği `redact_code()`'tan geçer (yalnızca bilinen anahtar biçimleri;
  `token = x` gibi kodu bozmasın diye anahtar=değer sezgisi yok). 1 MB üstü ve ikili dosya gösterilmez.
- Dosya simgeleri kendi tasarımımız (`lib/code/fileIcon.ts`, renkler token'dan); VS Code'un simge fontu kullanılmaz.
  Git hataları `git::friendly()` ile anlaşılır cümleye çevrilir (push izni yok, giriş gerekli, reddedildi…).
- Renklendirme Shiki 4.5.0 (`lib/code/highlight.ts`, sürüm sabit): JavaScript regex motoru (WASM yok, CSP'ye uyar),
  diller `shiki/langs/<dil>.mjs` ile ilk kullanımda yüklenir. Renkler `--shk-*` → CodeView'de tema token'larına
  bağlı (tema değişince yeniden renklendirme gerekmez). Satırlar sanal liste (18px); kaydırma açıkken tam liste.
- ✧ AI commit mesajı (`src/ai.rs`, `git_ai_message`): kullanıcının kendi Claude Code'u `claude -p "<istem>"
  --no-session-persistence --model haiku --tools ""`, stdin'de `git::changes_for_ai` (staged varsa yalnızca o, yoksa
  hepsi + izlenmeyen adlar + son 10 commit konusu; diff `redact_code()`'tan geçer, 60 KB'ta kesilir). Aboneliği
  kullanır; `--bare` KULLANMA (yalnızca API anahtarı görür). İstem `--tools`'tan önce gelmeli (değişken sayılı).
  Bu süreç `sessions/<pid>.json` yazar: `engine::mark_own_helper` ile listeden gizlenir. İç Claude env'i temizlenir.
- Dal seçici `lib/BranchPicker.svelte` (repo satırındaki `⑂ dal`): `git_branches` (`for-each-ref`, yerel + uzak) ve
  `git_checkout` (`switch`, `switch -c <ad> [temel]`, uzak dal → `switch --track`). Ad önce `check-ref-format
  --branch` ile denetlenir (`-` ile başlayamaz). Geçiş değişikliklerin üzerine yazacaksa "stash & switch" sorulur
  (`stash push -u`, geri açılmaz).
- Svelte `$effect` içinde `bind:this` ile gelen öğeyi okuma (`untrack`): öğe her yüklemede yeniden oluşursa efekt
  sonsuz döngüye girer ve sayfa donar.

## Sunucu (`server/`)
- `src/app.ts` uç noktalar (`/api/v1/...`), `src/db.ts` veritabanı arayüzü (üretimde `pg`, test/yerelde PGlite) ve
  şema adımları (`MIGRATIONS`: var olan adım değişmez, yenisi eklenir), `src/main.ts` başlatma.
- XP hesabı sunucuda, `packages/shared/src/xp.ts` ile (uygulamayla aynı kod). İstemci yalnızca `active` ve saat dilimi
  (`tz`: UTC'ye göre dakika, doğuya artı) gönderir; kredi atışlar arası geçen süreden (≤90 sn).
- Token yalnızca SHA-256 özeti olarak saklanır. Saat dilimi günde bir kez değişir. Hız sınırı IP başına
  (`CF-Connecting-IP`). Gizlilik: `hide_name` → "Anonymous", `hide_time` → süre/canlı nokta gösterilmez.
- Testler: `src/app.test.ts`, sahte saat (`now`) + bellekte PGlite. Bağımlılık sürümleri sabit (`--save-exact`).
- `src/friends.ts`: arkadaşlar (tek satır `a < b`, `pending`/`accepted`), mesajlar (yalnızca arkadaşlar, 2000 karakter),
  durum (`online`/`away`/`invisible`), WS `/ws` (Authorization başlığı; geçersizse 4401 ile kapanır). `src/live.ts`
  bellekte canlı bağlantı kaydı: "is now online" yalnızca 10+ dk yokluktan sonra. Görünmez → arkadaşlara çevrimdışı.
- Hile önlemi: heartbeat yalnızca WS açıkken kredi verir (`requireLive`, testlerde kapatılabilir).
- Kimlikler her yerde **metin** (`String(id)`): sürücüler bigint'i farklı döndürür, canlı kayıt bununla anahtarlanır.
- Testlerde WS: `app.injectWS('/ws', { headers }, { onInit })` (dinleyici açılmadan bağlanmalı, "hello" hemen gelir).
- Mesajlar uçtan uca şifreli: istemci `{to, ciphertext, nonce, toKey}` (hex) gönderir; sunucu düz metin kabul etmez,
  `toKey` alıcının güncel anahtarı değilse 409. Açık anahtar `PUT /api/v1/me/key`. Göç 4 eski düz mesajları siler.
- `src/news.ts`: duyurular (yalnızca `ADMIN_TOKEN` ile; yoksa yönetici uçları 404) canlı yayınlanır; geri bildirim
  name#tag ile saklanır. Yönetici aracı: `ADMIN_TOKEN=… SERVER=… npm run admin -- announce "başlık" "metin"`.
- Henüz yok: istek imzası.

- Canlı sunucu: `kaansterminal.karakuslabs.com` (Docker: `server/Dockerfile` + `server/deploy/`). Sunucu ayrıntıları
  yerelde; IP, anahtar yolu, sunucu adı repoya (kod, belge, commit mesajı) YAZILMAZ. Sunucuya yazmadan önce sor.

## Uygulama ↔ sunucu (`src/social.rs`, `lib/social.svelte.ts`)
- Bütün sunucu istekleri Rust'tan (`ureq`, sürüm sabit): token `<app_data>/account.json` (0600), sayfaya girmez; CSP
  değişmez. Sayfa yalnızca izin listesindeki uçları ister (`social_get`: status, leaderboard.week/all).
- Sunucu adresi `social::server_url()`: `KAANS_SERVER`, yoksa debug'da `http://127.0.0.1:8787`, release'de
  `https://kaansterminal.karakuslabs.com`. Sunucu yoksa uygulama yerel önizlemeyle çalışır ("offline").
- Cihaz özeti: donanım UUID'si (Mac `gethostuuid`, Windows MachineGuid) + tuz → SHA-256; ham kimlik gönderilmez.
- Heartbeat: engine döngüsü dakikalık örnekte `active`'i kanala yollar, `social` iş parçacığı gönderir
  (`social://status` / `social://error`). Tema `social_set_theme` ile bildirilir.
- İlk açılış: sunucu ayaktaysa ve kayıt yoksa hacker name penceresi (`HackerName.svelte`); "later" bir daha
  kendiliğinden açmaz (`social.later`). Sıralamada seçilen kişinin kartı onun temasında (`deriveTheme` kart içine).
- Canlı bağlantı `src/live.rs` (`tungstenite`, sürüm sabit): kayıtlıysa bağlanır, olaylar `social://live`, durum
  `social://live-state`; 30 sn ping (Cloudflare), sistem 5 dk boştaysa `away`, koparsa 2→60 sn beklemeyle yeniden.
- Arkadaşlar/mesajlar `lib/friends.svelte.ts`; görünümler `FriendsView.svelte` (sol sütun, `ui.friends`) ve
  `ChatDock.svelte` (sağ alt: bildirimler + cevap kutusu; tarayıcı demosunda sohbet kutuları). Sağ alt yığın App'teki
  `.corner` (uygulama uyarıları `Toasts` ile birlikte).
- Sohbet pencereleri: `chat_window` komutu → `chat-<hex(name#tag)>` etiketi, `index.html#chat=name#tag`
  (`chat/ChatApp.svelte`), farenin bulunduğu ekranın ortasında. Açık pencere localStorage'da `chat.win.<k>`'yı 5 sn'de
  bir tazeler; ana pencere o arkadaş için köşe bildirimi göstermez.
- Uçtan uca şifreleme `src/e2e.rs` (`crypto_box`, sürüm sabit): anahtar `<app_data>/keys.json` (0600). Şifreleme ve
  çözme yalnızca Rust'ta (`Social::send_message`, `open_message`; geçmiş `get("messages?…")` ve canlı olay
  `live.rs`'de çözülür). Sayfa yalnızca `body` (çözülemezse null + `locked`) görür.
- Duyurular `lib/news.svelte.ts` + `NewsView.svelte` (zil, `ui.news`; okunmamış = son görülen kimlik), geri bildirim
  `FeedbackDialog.svelte` (activity bar'ın altındaki parlayan buton). Bağlantılar `RichText.svelte` (`richText.ts`:
  http(s) ve @ad → GitHub; HTML yorumlanmaz), tarayıcıda açılır.
- Uçtan uca test: `server/` içinde boş veritabanıyla `PORT=18787 npm run dev`, sonra
  `KAANS_TEST_SERVER=http://127.0.0.1:18787 cargo test -- live_` (kayıt/heartbeat + arkadaşlık/mesaj/canlı olay).

## Dağıtım (`src/updates.rs`, `src/watchdog.rs`, Aşama 7)
- Güncelleme: `tauri-plugin-updater` (=2.14.0, yalnızca Rust'tan; sayfada eklenti yok). `latest.json` GitHub
  Releases'ten (`kaankarakusx/kaans-terminal`, `releases/latest/download/latest.json`). Açık anahtar
  `tauri.conf.json` → `plugins.updater.pubkey`; özel anahtar `~/.tauri/kaans-terminal.key`, şifresi Anahtar
  Zinciri'nde (`kaans-terminal-updater`). **Anahtarı/şifreyi asla ekrana, dosyaya, commit'e yazma.**
- 20 sn sonra + 15 dk'da bir + uykudan dönüşte kontrol, arka planda iner (`update://state`), "restart to update" ya
  da sistem 10 dk boştaysa kurulur (terminaller host'ta yaşar). Notlar `<app_data>/whats-new.json` → ilk açılışta
  `WhatsNew.svelte`. Debug'da kapalı (`KAANS_UPDATE_TEST=1` açar). Ayarlar → ABOUT (`AboutTab.svelte`).
- Sürüm: `tauri.conf.json` version + kök `Cargo.toml` version (aynı), `src-tauri/release-notes.md`, sonra
  `node apps/desktop/src-tauri/scripts/release.mjs` (imzalı derler, `latest.json` yazar, YAYINLAMAZ; gh komutunu
  basar). `createUpdaterArtifacts` yalnızca `tauri.release.conf.json`'da (düz `tauri build` anahtarsız çalışsın).
- Lisanslar: `node apps/desktop/src-tauri/scripts/licenses.mjs` → `resources/licenses/THIRD-PARTY.txt` (bağımlılık
  değişince yeniden üret). README.md, PRIVACY.md kökte; gizlilik metni koda uymalı (yeni veri sunucuya giderse güncelle).
- Hata günlüğü/bekçi (`watchdog.rs`): `<app_data>/errors.log` (sayfa hataları, Rust panikleri backtrace'li, donmalar).
  Nabızlar: `engine` (30 sn), `main` (Rust UI iş parçacığı, 8 sn; Mac'te `sample` ile yığın `<app_log>/hang-*.txt`),
  `ui:<pencere>` (`ui_beat` 2 sn'de bir, yalnızca görünür+öndeyken; 10 sn). Release'te ana sayfa 30 sn donarsa
  yeniden yüklenir.

## Token savers (`src/savers.rs`)
- rtk: yalnızca **rtk-oss** (Apache-2.0; varsayılan rtk BUSL-1.1 motor içerir). Sürüm ve SHA-256
  `src-tauri/scripts/fetch-rtk.mjs` içinde sabit; ikili dosyalar git'e girmez, `resources/bin/<hedef>/` altına
  indirilir (`beforeBuildCommand` çalıştırır; elle: `node src-tauri/scripts/fetch-rtk.mjs`). Lisans `resources/licenses/`.
- Kullanıcının PATH'teki rtk'si öncelikli (dokunulmaz). Yoksa paketteki kopya `~/.local/bin/rtk`'ya konur; kaydı
  `<app_data>/rtk-managed.json`. Yerinde bizim olmayan dosya varsa ezilmez; uygulama yalnızca kendi kopyasını günceller.
- rtk hook'u çıplak `rtk` çağırır: kendi kopyamız kuruluysa uygulama terminallerinin PATH'ine `~/.local/bin` eklenir.
- Kur/kaldır rtk'nin kendi komutlarıyla: `rtk init -g [--codex | --agent antigravity] --auto-patch` / `--uninstall`,
  `RTK_TELEMETRY_DISABLED=1`, stdin boş; önce bizim yedeğimiz.
- Gerçek rtk testi geçici HOME'da çalışır (`savers::tests::real_rtk_…`); rtk indirilmemişse atlanır.
- caveman v4: `claude plugin marketplace add JuliusBrussee/caveman` + `claude plugin install caveman@caveman`
  (Gemini varsa `gemini extensions install …`); kaldırma `plugin uninstall`; marketplace'i biz eklediysek o da kaldırılır.

## Terminal katmanı (`src/host/`)
- Terminal kabukları başlatılırken Claude Code'un iç değişkenleri (`CLAUDECODE`, `CLAUDE_CODE_*`, `CLAUDE_PID`…;
  `host::server::is_claude_internal`) temizlenir: uygulama bir Claude oturumundan başlatıldıysa (geliştirme) terminalde
  açılan Claude kendini alt oturum sanıp kayıt tutmaz ve sohbet olarak görünmez. Süreç adı sürüm numarasıysa
  (`2.1.319`) "claude" gösterilir.
- Oturum sunucusu **aynı ikili dosyanın** `--session-host <soket>` kipidir (sidecar yok). Uygulama yoksa başlatır;
  `setsid` ile ayrılır, uygulama kapansa/güncellense de PTY'ler yaşar. Oturum ve istemci kalmayınca 60 sn sonra kapanır.
- Soket: Mac'te `$TMPDIR/kaans-terminal-host.sock` (0600), Windows'ta namespaced pipe. Günlük: `$TMPDIR/kaans-terminal-host.log`.
- Protokol `host/protocol.rs`: `[u32 uzunluk][u8 tür][u16 id uzunluğu][id][veri]`; kontrol JSON, girdi/çıktı ham bayt.
  Protokolü değiştirince `PROTOCOL_VERSION`'ı artır (eski sunucu oturumsuzsa kapanıp yenisi başlar).
- `Control` tek paylaşılan bağlantı; her ekran bölmesi ayrı `Attachment` bağlantısı (geçmiş yalnızca ona oynatılır).
  Ayrılma `Detach` → `Detached` el sıkışmasıyla biter (interprocess'te yarım kapatmak bağlantıyı kapatmaz).
- Geçmiş: oturum başına 2 MB; kesilmişse ilk satır sonundan oynatılır. DEC 2026 senkron çıktı işaretleri olduğu gibi
  geçer (screen yok, röleye gerek yok).
- End Session: SIGHUP (ön plan grubu + kabuk) → 1,2 sn → SIGTERM → 1,5 sn → SIGKILL; son bilinen alt süreçler dahil.
  `Exit { ended: true }` haritada patlama demektir.
- Kalıntı (leftover): kabuk kapanınca son anlık görüntüdeki alt süreçlerden yaşayanlar. Yalnızca sunucunun listesindeki
  pid'ler öldürülebilir; `danger_kill` de yalnızca kaçak listesindeki pid'i öldürür.
- Elle deneme (uygulama açıkken): `cargo run --example host_cli -- list|create <cwd> "<komut>"|replay <id>|kill <id>|kill-leftover <pid>`,
  uçtan uca: `cargo run --example host_probe`.

## Terminal penceresi (`web/src/term/`)
- Pencere etiketi `term-*` (capabilities'te izinli); adres `index.html#term=<etiket>&open=<oturum>`. `main.ts` hash'e
  göre `TermApp` ya da ana paneli bağlar.
- Sekme/bölme durumu `localStorage` (`term.win.<etiket>`, dizin `term.windows`). Açılışta pencereler kendiliğinden açılmaz; oturum
  kart/araba ile açılınca penceresi kayıtlı düzeniyle (`windowFor`) gelir.
- Uygulama menüsünde "Close Window ⌘W" yok (bkz. `lib.rs::app_menu`): ⌘W bölmeyi kapatır, oturum yaşar.
- Option karakter üretir (`macOptionIsMeta: false`); ⌥←/→/⌫ ve Shift+Return `TerminalPane` içinde elle eşlenir.
  Resim yapıştırma Claude'a Ctrl+V gönderir (Claude panoyu kendisi okur).
- Chrome'da ⌘T/⌘D gibi kısayolları tarayıcı yakalar; terminal kısayollarını gerçek uygulamada dene.
- Tarayıcı otomasyonunda "type" eylemi xterm'e ulaşmaz; tek tek tuş gönder.

## Uygulama ↔ sayfa köprüsü
- Rust → sayfa: her saniye `engine://state` (`Snapshot`), yeni olay varsa `engine://events` (`EngineEvent[]`),
  güç değişince `power://state`. Adlar `lib.rs` ve `web/src/lib/engine.svelte.ts` içinde aynı olmalı.
- Sayfa → Rust: `invoke('engine_initial')` son snapshot'ı, son 200 olayı ve güç durumunu verir.
- Yeni olay dinlemek için `engine.onEvents(fn)` (orbit.map okları böyle beslenir).
- Rust tipleri `serde(rename_all = "camelCase")`; TS karşılıkları `web/src/lib/types.ts`. Birini değiştirirsen
  ötekini de değiştir (ileride `packages/shared`).

## Test
- Engine mantığı (durum hesabı `statusOf`, fork tekrarları, compaction, alt ajanlar, maliyet, same file) için
  `fixtures/` altındaki örnek dosyalarla birim testi yaz. Kullanıcının gerçek `~/.claude` verisine testte bağlanma.
- Testler gerçek dosya sistemine (örn. `/tmp/<proje>`) yazmaz; her zaman `tempfile::tempdir()`.
- Redaction için bilinen anahtar biçimleriyle test tablosu.
- Tema: `derive.test.ts` varsayılan temanın türetilmiş token'larının `tokens.css` ile birebir aynı olduğunu
  sınar. `tokens.css`'e ya da `default.json`'a renk eklerken ikisini birlikte güncelle.
- Windows'a özel kod Mac'te Tauri build script'i yüzünden `cargo check --target` ile denetlenemez; modülü küçük bir
  geçici crate'e `#[path]` ile alıp `cargo clippy --target x86_64-pc-windows-msvc` çalıştır.

## Dil ve iletişim
- Kullanıcıyla Türkçe konuş. Kod yorumları Türkçe olabilir; tanımlayıcılar (değişken, fonksiyon) İngilizce.
