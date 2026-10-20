//! First derivatives only, used by XPBD; avoids allocating full element Hessians.
use crate::math::Scalar;
use std::ops::{Add, Div, Mul, Neg, Sub};
#[derive(Clone, Copy)]
pub(super) struct Dual<const N: usize> {
    pub v: f64,
    pub g: [f64; N],
}
impl<const N: usize> Dual<N> {
    pub fn variable(v: f64, i: usize) -> Self {
        let mut a = Self::c(v);
        a.g[i] = 1.;
        a
    }
    fn chain(self, v: f64, d: f64) -> Self {
        Self {
            v,
            g: self.g.map(|x| x * d),
        }
    }
}
impl<const N: usize> Scalar for Dual<N> {
    fn c(v: f64) -> Self {
        Self { v, g: [0.; N] }
    }
    fn value(self) -> f64 {
        self.v
    }
    fn sqrt(self) -> Self {
        let r = self.v.sqrt();
        self.chain(r, 0.5 / r)
    }
    fn ln(self) -> Self {
        self.chain(self.v.ln(), 1. / self.v)
    }
}
impl<const N: usize> Add for Dual<N> {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self {
            v: self.v + b.v,
            g: std::array::from_fn(|i| self.g[i] + b.g[i]),
        }
    }
}
impl<const N: usize> Neg for Dual<N> {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            v: -self.v,
            g: self.g.map(|x| -x),
        }
    }
}
impl<const N: usize> Sub for Dual<N> {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        self + -b
    }
}
#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize> Mul for Dual<N> {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self {
            v: self.v * b.v,
            g: std::array::from_fn(|i| self.g[i] * b.v + self.v * b.g[i]),
        }
    }
}
#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize> Div for Dual<N> {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        self * b.chain(1. / b.v, -1. / b.v.powi(2))
    }
}
