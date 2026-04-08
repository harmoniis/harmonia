//! Human-like timing simulation and randomization helpers.

use std::time::Duration;

/// Simple seed from system time (avoids rand dependency).
pub fn simple_seed() -> usize {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| (d.as_nanos() as usize) ^ (d.as_millis() as usize))
        .unwrap_or(42)
}

/// Generate a random-ish value in [min, max].
pub fn random_range(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let seed = simple_seed() as u64;
    min + (seed % (max - min + 1))
}

/// Simulate a human-like delay (50-300ms).
pub fn human_delay() {
    let ms = random_range(50, 300);
    std::thread::sleep(Duration::from_millis(ms));
}

/// Simulate a short action delay (20-100ms) for rapid interactions.
pub fn short_delay() {
    let ms = random_range(20, 100);
    std::thread::sleep(Duration::from_millis(ms));
}

/// Simulate a page load wait (500-2000ms).
pub fn page_load_delay() {
    let ms = random_range(500, 2000);
    std::thread::sleep(Duration::from_millis(ms));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_range_in_bounds() {
        for _ in 0..50 {
            let v = random_range(50, 300);
            assert!(v >= 50 && v <= 300, "value {} out of range", v);
        }
    }

    #[test]
    fn random_range_equal_bounds() {
        assert_eq!(random_range(42, 42), 42);
    }
}
