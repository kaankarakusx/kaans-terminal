# Engine fixture'ı

Sahte bir `$CLAUDE_DIR`. Gerçek Claude Code dosyalarının biçimini taklit eder; içeriği elle yazılmıştır,
gerçek kullanıcı verisi içermez. `src/engine/tests.rs` içindeki `static_fixture_*` testleri kullanır.

- `sessions/4242.json`: canlı kayıt (`waiting`)
- `sessions/4242.0123abcd.key`: engine'in **asla açmaması** gereken dosya (içeriği geçerli JSON bile değil)
- `projects/-tmp-fixture-app/fx-main.jsonl`: ana transcript (2026-01-15, UTC 11:00–12:20)
  - bölünmüş assistant mesajı (aynı `message.id`, iki satır), hatalı araç sonucu, web çağrısı, gizli bilgi
- `projects/-tmp-fixture-app/fx-main/subagents/agent-afx1.{jsonl,meta.json}`: çalışan bir alt ajan
