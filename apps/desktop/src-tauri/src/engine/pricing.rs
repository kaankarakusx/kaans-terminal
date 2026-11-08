//! "api-eq" maliyeti: aynı token'ların API liste fiyatlarıyla karşılığı (MIMARI.md §3.4).
//! Abonelik faturası değildir. Tablo `resources/pricing.json` dosyasındadır; kodda fiyat yazılmaz.

use super::transcript::Tokens;
use serde::Deserialize;
use std::path::Path;

const BUNDLED: &str = include_str!("../../resources/pricing.json");
const PER_TOKEN: f64 = 1.0 / 1_000_000.0;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Defaults {
    cache_read_mult: f64,
    cache_write5m_mult: f64,
    cache_write1h_mult: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LongContext {
    /// İstemin bağlamı bu değeri aşarsa…
    pub threshold: u64,
    /// …bütün oranlar bu çarpanla büyür.
    pub multiplier: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPrice {
    pub prefix: String,
    pub input: f64,
    pub output: f64,
    #[serde(default)]
    pub cache_read: Option<f64>,
    pub context_window: u64,
    #[serde(default)]
    pub long_context: Option<LongContext>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriceTable {
    defaults: Defaults,
    models: Vec<ModelPrice>,
}

impl PriceTable {
    /// Uygulamayla gelen tablo.
    pub fn bundled() -> Self {
        serde_json::from_str(BUNDLED).expect("resources/pricing.json geçersiz")
    }

    /// Kullanıcının üzerine yazdığı tablo varsa o, yoksa (ya da okunamazsa) gömülü tablo.
    pub fn load_or_bundled(override_path: &Path) -> Self {
        std::fs::read(override_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(Self::bundled)
    }

    /// En uzun önek eşleşmesi (`claude-opus-5-5` `claude-opus-5`'ten önce gelir). Bilinmeyen model: `None`.
    pub fn find(&self, model: &str) -> Option<&ModelPrice> {
        self.models
            .iter()
            .filter(|m| model.starts_with(&m.prefix))
            .max_by_key(|m| m.prefix.len())
    }

    pub fn context_window(&self, model: &str) -> Option<u64> {
        self.find(model).map(|m| m.context_window)
    }

    /// Bir isteğin (ya da birkaç isteğin toplamının) maliyeti. `context` uzun bağlam eşiği için istemin boyutudur.
    pub fn cost(&self, model: &str, t: &Tokens, context: u64) -> Option<f64> {
        let p = self.find(model)?;
        let mult = p
            .long_context
            .as_ref()
            .filter(|lc| context > lc.threshold)
            .map_or(1.0, |lc| lc.multiplier);
        let input = p.input * mult;
        let cache_read = p
            .cache_read
            .unwrap_or(p.input * self.defaults.cache_read_mult)
            * mult;
        let write_1h = t.cache_write_1h.min(t.cache_write);
        let write_5m = t.cache_write - write_1h;
        let usd = t.input as f64 * input
            + t.output as f64 * p.output * mult
            + t.cache_read as f64 * cache_read
            + write_5m as f64 * input * self.defaults.cache_write5m_mult
            + write_1h as f64 * input * self.defaults.cache_write1h_mult;
        Some(usd * PER_TOKEN)
    }

    /// Önbellekten okunan token'lar normal girdi fiyatıyla okunsaydı ne kadar fazla ödenirdi.
    pub fn cache_savings(&self, model: &str, t: &Tokens) -> Option<f64> {
        let p = self.find(model)?;
        let cache_read = p
            .cache_read
            .unwrap_or(p.input * self.defaults.cache_read_mult);
        Some(t.cache_read as f64 * (p.input - cache_read).max(0.0) * PER_TOKEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(input: u64, output: u64, cache_read: u64, cache_write: u64, w1h: u64) -> Tokens {
        Tokens {
            input,
            output,
            cache_read,
            cache_write,
            cache_write_1h: w1h,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn bundled_table_parses_and_matches_longest_prefix() {
        let t = PriceTable::bundled();
        assert_eq!(t.find("claude-opus-5-5").unwrap().prefix, "claude-opus-5-5");
        assert_eq!(t.find("claude-opus-5").unwrap().prefix, "claude-opus-5");
        assert_eq!(
            t.find("claude-fable-5-1").unwrap().prefix,
            "claude-fable-5-1"
        );
        assert_eq!(t.find("claude-fable-5").unwrap().prefix, "claude-fable-5");
        assert_eq!(
            t.find("claude-sonnet-4-5-20250929").unwrap().prefix,
            "claude-sonnet-4-5"
        );
        assert!(t.find("gpt-5").is_none(), "bilinmeyen model fiyatsız");
        assert!(t.cost("mystery", &Tokens::default(), 0).is_none());
    }

    #[test]
    fn opus_5_5_cost() {
        let t = PriceTable::bundled();
        // 1M input $4 + 1M output $20 + 1M cache read $0.20 + 1M 5dk yazma $5 + 1M 1sa yazma $8
        let c = t
            .cost(
                "claude-opus-5-5",
                &tok(1_000_000, 1_000_000, 1_000_000, 2_000_000, 1_000_000),
                0,
            )
            .unwrap();
        assert!(close(c, 4.0 + 20.0 + 0.20 + 5.0 + 8.0), "{c}");
    }

    #[test]
    fn default_cache_read_multiplier() {
        let t = PriceTable::bundled();
        // Opus 4.8: cacheRead verilmemiş → $5 × 0.1
        let c = t
            .cost("claude-opus-4-8", &tok(0, 0, 1_000_000, 0, 0), 0)
            .unwrap();
        assert!(close(c, 0.5));
    }

    #[test]
    fn haiku_long_context_multiplier() {
        let t = PriceTable::bundled();
        let one = tok(1_000_000, 1_000_000, 0, 0, 0);
        assert!(close(
            t.cost("claude-haiku-5-5", &one, 100_000).unwrap(),
            0.10 + 0.50
        ));
        assert!(close(
            t.cost("claude-haiku-5-5", &one, 100_001).unwrap(),
            0.50 + 2.50
        ));
    }

    #[test]
    fn cache_savings() {
        let t = PriceTable::bundled();
        let s = t
            .cache_savings("claude-opus-5-5", &tok(0, 0, 1_000_000, 0, 0))
            .unwrap();
        assert!(close(s, 3.80));
    }

    #[test]
    fn context_windows() {
        let t = PriceTable::bundled();
        assert_eq!(t.context_window("claude-opus-5-5"), Some(1_000_000));
        assert_eq!(t.context_window("claude-haiku-4-5"), Some(200_000));
        assert_eq!(t.context_window("x"), None);
    }
}
