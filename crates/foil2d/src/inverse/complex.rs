//! Double-precision complex numbers with the arithmetic gfortran generates
//! for XFOIL's `COMPLEX` variables: products `(ac − bd, ad + bc)`, quotients
//! by Smith's range-reducing method, a real factor or divisor applied to
//! each part, and `EXP`, `LOG`, `ABS` as the C library's `cexp`, `clog`,
//! `cabs`.

use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }

    /// Real factor times the number (Fortran `R*Z`).
    pub fn scale(self, r: f64) -> Self {
        Self::new(r * self.re, r * self.im)
    }

    /// Division by a real number (Fortran `Z/R`).
    pub fn div_real(self, r: f64) -> Self {
        Self::new(self.re / r, self.im / r)
    }

    /// Modulus (`cabs`).
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// `cexp`.
    pub fn exp(self) -> Self {
        let e = self.re.exp();
        let (s, c) = self.im.sin_cos();
        Self::new(e * c, e * s)
    }

    /// Argument (the imaginary part of `clog`).
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
}

impl Add for C64 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for C64 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.re - o.re, self.im - o.im)
    }
}

impl Neg for C64 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.re, -self.im)
    }
}

impl Mul for C64 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

impl Div for C64 {
    type Output = Self;
    /// Smith's algorithm, as GCC expands Fortran complex division.
    fn div(self, o: Self) -> Self {
        let (a, b, c, d) = (self.re, self.im, o.re, o.im);
        if c.abs() < d.abs() {
            let ratio = c / d;
            let div = c * ratio + d;
            Self::new((a * ratio + b) / div, (b * ratio - a) / div)
        } else {
            let ratio = d / c;
            let div = d * ratio + c;
            Self::new((b * ratio + a) / div, (b - a * ratio) / div)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        let a = C64::new(1.0, 2.0);
        let b = C64::new(-3.0, 0.5);
        assert_eq!(a * b, C64::new(-4.0, -5.5));
        let q = (a * b) / b;
        assert!((q - a).abs() < 1e-15);
        let q = (a * C64::new(0.1, -7.0)) / C64::new(0.1, -7.0);
        assert!((q - a).abs() < 1e-15);
        assert_eq!(a.conj(), C64::new(1.0, -2.0));
        assert!((C64::new(0.0, std::f64::consts::PI).exp() - C64::new(-1.0, 0.0)).abs() < 1e-15);
        assert_eq!(C64::new(0.0, 1.0).arg(), std::f64::consts::FRAC_PI_2);
        assert_eq!(C64::new(3.0, 4.0).abs(), 5.0);
        assert_eq!(-a + a.scale(2.0), a);
        assert_eq!(a.div_real(2.0), C64::new(0.5, 1.0));
    }
}
