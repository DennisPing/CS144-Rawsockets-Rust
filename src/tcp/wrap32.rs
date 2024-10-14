use std::ops::Add;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wrap32 {
    value: u32,
}

impl Wrap32 {
    const WRAP_SIZE: u64 = 1 << 32;
    const HALF_WRAP: u64 = 1 << 31;

    pub fn new(value: u32) -> Self {
        Wrap32 { value }
    }

    /// Wrap an absolute `seq_no` given an `initial seq_no`
    pub fn wrap(n: u64, isn: Wrap32) -> Self {
        Wrap32::new((n + isn.value as u64) as u32)
    }

    /// Unwrap the given `initial seq_no` to an absolute `seq_no` closest to the `checkpoint`
    pub fn unwrap(&self, isn: Wrap32, checkpoint: u64) -> u64 {
        // ChatGPT black magic optimization :)

        // Calculate the relative sequence number
        let relative = self.value.wrapping_sub(isn.value) as u64;

        // Calculate the number of wraps `k` to get closest to checkpoint using bitwise shift
        let k = (checkpoint + Self::HALF_WRAP).saturating_sub(relative) >> 32;

        // Calculate the absolute sequence number
        relative + k * Self::WRAP_SIZE
    }
}

impl Add for Wrap32 {
    type Output = Wrap32;

    fn add(self, other: Wrap32) -> Wrap32 {
        Wrap32::new(self.value.wrapping_add(other.value))
    }
}

// -- Unit tests --

#[cfg(test)]
mod tests {
    use rand::distributions::Distribution;
    use rand::distributions::Uniform;
    use rand::Rng;
    use rayon::prelude::*;
    use super::*;

    // -- Test wrapping --

    #[test]
    fn test_wrap_zero_offset() {
        let seq_no = 3 * (1u64 << 32);
        let isn = Wrap32::new(0);

        let wrapped = Wrap32::wrap(seq_no, isn);
        assert_eq!(wrapped, Wrap32::new(0));
    }

    #[test]
    fn test_wrap_positive_offset() {
        let seq_no = 3 * (1u64 << 32) + 17;
        let isn = Wrap32::new(15);

        let wrapped = Wrap32::wrap(seq_no, isn);
        assert_eq!(wrapped, Wrap32::new(32));
    }

    #[test]
    fn test_wrap_negative_offset() {
        let seq_no = 7 * (1u64 << 32) - 2;
        let isn = Wrap32::new(15);

        let wrapped = Wrap32::wrap(seq_no, isn);
        assert_eq!(wrapped, Wrap32::new(13));
    }

    // -- Test unwrapping --

    #[test]
    fn test_unwrap_first_sequence_after_isn_zero() {
        let unwrapped = Wrap32::new(1).unwrap(Wrap32::new(0), 0);
        assert_eq!(unwrapped, 1u64)
    }

    #[test]
    fn test_unwrap_first_sequence_after_first_wraparound() {
        let unwrapped = Wrap32::new(1).unwrap(Wrap32::new(0), u32::MAX as u64);
        assert_eq!(unwrapped, (1u64 << 32) + 1)
    }

    #[test]
    fn test_unwrap_last_sequence_before_third_wraparound() {
        let unwrapped = Wrap32::new(u32::MAX - 1).unwrap(Wrap32::new(0), 3 * (1u64 << 32));
        assert_eq!(unwrapped, 3 * (1u64 << 32) - 2);
    }

    #[test]
    fn test_unwrap_tenth_before_third_wraparound() {
        let unwrapped = Wrap32::new(u32::MAX - 10).unwrap(Wrap32::new(0), 3 * (1u64 << 32));
        assert_eq!(unwrapped, 3 * (1u64 << 32) - 11);
    }

    #[test]
    fn test_unwrap_with_nonzero_isn() {
        let unwrapped = Wrap32::new(u32::MAX).unwrap(Wrap32::new(10), 3 * (1u64 << 32));
        assert_eq!(unwrapped, 3 * (1u64 << 32) - 11);
    }

    #[test]
    fn test_unwrap_max_wrapped_value_with_zero_isn() {
        let unwrapped = Wrap32::new(u32::MAX).unwrap(Wrap32::new(0), 0);
        assert_eq!(unwrapped, u32::MAX as u64);
    }

    #[test]
    fn test_unwrap_value_equals_isn_returns_zero() {
        let unwrapped = Wrap32::new(16).unwrap(Wrap32::new(16), 0);
        assert_eq!(unwrapped, 0u64);
    }

    #[test]
    fn test_unwrap_max_value_with_nonzero_isn() {
        let unwrapped = Wrap32::new(15).unwrap(Wrap32::new(16), 0);
        assert_eq!(unwrapped, u32::MAX as u64);
    }

    #[test]
    fn test_unwrap_zero_value_with_large_isn() {
        let unwrapped = Wrap32::new(0).unwrap(Wrap32::new(i32::MAX as u32), 0);
        assert_eq!(unwrapped, (i32::MAX as u64) + 2);
    }

    #[test]
    fn test_unwrap_max_value_with_max_isn_returns_half_wrap() {
        let unwrapped = Wrap32::new(u32::MAX).unwrap(Wrap32::new(i32::MAX as u32), 0);
        assert_eq!(unwrapped, 1u64 << 31);
    }

    #[test]
    fn test_unwrap_max_value_with_half_wrap_isn() {
        let unwrapped = Wrap32::new(u32::MAX).unwrap(Wrap32::new(1 << 31), 0);
        assert_eq!(unwrapped, (u32::MAX as u64) >> 1);
    }

    // -- Test `+` operator overload --

    #[test]
    fn test_add() {
        let x = Wrap32::new(1);
        let y = Wrap32::new(2);
        let z = Wrap32::new(3);
        assert_eq!(x + y, z);
    }

    #[test]
    fn test_add_overflow() {
        let x = Wrap32::new(u32::MAX);
        let y = Wrap32::new(1);
        let z = Wrap32::new(0);
        assert_eq!(x + y, z);
    }

    // -- Test compare --

    #[test]

    fn test_equality() {
        let wrap_a = Wrap32::new(3);
        let wrap_b = Wrap32::new(1);

        assert_ne!(wrap_a, wrap_b);
        assert_eq!(wrap_a != wrap_b, true);
        assert_eq!(wrap_a == wrap_b, false);
    }

    #[test]
    fn test_equality_random() {
        let n_reps = 32768;
        let mut rng = rand::thread_rng();
        for _ in 0..n_reps {
            let n: u32 = rng.gen();
            let diff: u8 = rng.gen();
            let m: u32 = n + diff as u32;

            let wrap_n = Wrap32::new(n);
            let wrap_m = Wrap32::new(m);

            assert_eq!(wrap_n == wrap_m, n == m);
            assert_eq!(wrap_n != wrap_m, n != m);
        }
    }

    // -- Test roundtrip --

    #[test]
    fn test_roundtrip() {
        fn check_roundtrip(isn: Wrap32, value: u64, checkpoint: u64) {
            assert_eq!(Wrap32::wrap(value, isn).unwrap(isn, checkpoint), value)
        }

        let n_reps = 1_000_000;
        let dist31minus1 = Uniform::from(0u32..=(1u32 << 31) - 1);
        let dist32 = Uniform::from(0u32..=u32::MAX);
        let big_offset: u64 = (1u64 << 31) - 1;
        let dist63 = Uniform::from(big_offset..=(1u64 << 63));

        // Run parallel tests because we don't have all the time in the world
        (0..n_reps).into_par_iter().for_each(|_| {
            let mut rng = rand::thread_rng();
            let isn_value = dist32.sample(&mut rng);
            let isn = Wrap32::new(isn_value);
            let val = dist63.sample(&mut rng);
            let offset = dist31minus1.sample(&mut rng) as u64;

            check_roundtrip(isn, val, val);
            check_roundtrip(isn, val + 1, val);
            check_roundtrip(isn, val - 1, val);
            check_roundtrip(isn, val + offset, val);
            check_roundtrip(isn, val - offset, val);
            check_roundtrip(isn, val + big_offset, val);
            check_roundtrip(isn, val - big_offset, val);
        });
    }
}