use chrono::{DateTime, SecondsFormat, Utc};

/// Current time as ISO-8601 UTC with millisecond precision. Lexicographic
/// order of these strings matches chronological order.
pub fn now_iso() -> String {
    iso(Utc::now())
}

pub fn iso(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// Sortable, time-ordered id.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// UTF-16 code-unit offset of a byte index, so spans can be used directly
/// with JavaScript string slicing.
pub fn utf16_offset(text: &str, byte_idx: usize) -> usize {
    text[..byte_idx].encode_utf16().count()
}

/// Tiny deterministic PRNG (xorshift64*) so resurfacing exploration can be
/// seeded in tests without pulling in a dependency.
pub struct Rng(u64);

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    pub fn from_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng::seeded(nanos ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_count_surrogate_pairs() {
        let s = "a😀b";
        let b = s.find('b').unwrap();
        assert_eq!(utf16_offset(s, b), 3);
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut a = Rng::seeded(42);
        let mut b = Rng::seeded(42);
        for _ in 0..100 {
            let x = a.next_f64();
            assert_eq!(x, b.next_f64());
            assert!((0.0..1.0).contains(&x));
        }
    }
}
