use crate::model::{FEIGENBAUM_ALPHA, FEIGENBAUM_DELTA, PHI};

pub fn seeded_weight(a: usize, b: usize, scale: f64) -> f64 {
    let x = ((a + 1) as f64 * PHI + (b + 1) as f64 / FEIGENBAUM_DELTA).sin()
        + ((a + 1) as f64 / FEIGENBAUM_ALPHA + (b + 1) as f64 * 0.5).cos();
    clamp(x * 0.5 * scale, -scale, scale)
}

pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

pub fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn digest_hex(digest: u64) -> String {
    format!("{digest:016x}")
}
