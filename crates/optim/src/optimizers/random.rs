//! Seeded pseudo-random numbers: xoshiro256++ (Blackman and Vigna)
//! initialized by SplitMix64.

/// Pseudo-random generator; the same seed gives the same sequence.
#[derive(Debug, Clone)]
pub(crate) struct Random {
    s: [u64; 4],
    spare: Option<f64>,
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        let mut state = seed;
        Self {
            s: std::array::from_fn(|_| splitmix64(&mut state)),
            spare: None,
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)`.
    pub(crate) fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard normal (Box–Muller).
    pub(crate) fn normal(&mut self) -> f64 {
        if let Some(z) = self.spare.take() {
            return z;
        }
        let u1 = 1.0 - self.uniform(); // (0, 1]
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let (s, c) = (std::f64::consts::TAU * u2).sin_cos();
        self.spare = Some(r * s);
        r * c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_sequence() {
        // xoshiro256++ from the state 1, 2, 3, 4 (reference implementation).
        let mut r = Random {
            s: [1, 2, 3, 4],
            spare: None,
        };
        let v: Vec<u64> = (0..3).map(|_| r.next_u64()).collect();
        assert_eq!(v, [41943041, 58720359, 3588806011781223]);
    }

    #[test]
    fn reproducible_and_distributed() {
        let a: Vec<f64> = {
            let mut r = Random::new(7);
            (0..10).map(|_| r.normal()).collect()
        };
        let mut r = Random::new(7);
        assert_eq!(a, (0..10).map(|_| r.normal()).collect::<Vec<_>>());
        let mut other = Random::new(8);
        assert_ne!(a[0], other.normal());

        let n = 200_000;
        let mut r = Random::new(1);
        let u: Vec<f64> = (0..n).map(|_| r.uniform()).collect();
        assert!(u.iter().all(|v| (0.0..1.0).contains(v)));
        let z: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let mean = z.iter().sum::<f64>() / n as f64;
        let var = z.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(
            mean.abs() < 0.01 && (var - 1.0).abs() < 0.01,
            "{mean} {var}"
        );
    }
}
