//! Small second-order forward AD for element energies, not numerical derivatives.
use std::ops::{Add, Div, Mul, Neg, Sub};

pub trait Scalar:
    Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    fn c(v: f64) -> Self;
    fn value(self) -> f64;
    fn sqrt(self) -> Self;
    fn ln(self) -> Self;
}

impl Scalar for f64 {
    fn c(v: f64) -> Self {
        v
    }
    fn value(self) -> f64 {
        self
    }
    fn sqrt(self) -> Self {
        self.sqrt()
    }
    fn ln(self) -> Self {
        self.ln()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Jet<const N: usize> {
    pub v: f64,
    pub g: [f64; N],
    pub h: [[f64; N]; N],
}

impl<const N: usize> Jet<N> {
    pub fn variable(v: f64, i: usize) -> Self {
        let mut x = Self::c(v);
        x.g[i] = 1.0;
        x
    }
    fn chain(self, value: f64, first: f64, second: f64) -> Self {
        Self {
            v: value,
            g: std::array::from_fn(|i| first * self.g[i]),
            h: std::array::from_fn(|i| {
                std::array::from_fn(|j| first * self.h[i][j] + second * self.g[i] * self.g[j])
            }),
        }
    }
}

impl<const N: usize> Scalar for Jet<N> {
    fn c(v: f64) -> Self {
        Self {
            v,
            g: [0.0; N],
            h: [[0.0; N]; N],
        }
    }
    fn value(self) -> f64 {
        self.v
    }
    fn sqrt(self) -> Self {
        let r = self.v.sqrt();
        self.chain(r, 0.5 / r, -0.25 / (r * r * r))
    }
    fn ln(self) -> Self {
        self.chain(self.v.ln(), 1.0 / self.v, -1.0 / self.v.powi(2))
    }
}
impl<const N: usize> Add for Jet<N> {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self {
            v: self.v + b.v,
            g: std::array::from_fn(|i| self.g[i] + b.g[i]),
            h: std::array::from_fn(|i| std::array::from_fn(|j| self.h[i][j] + b.h[i][j])),
        }
    }
}
impl<const N: usize> Neg for Jet<N> {
    type Output = Self;
    fn neg(self) -> Self {
        self * Self::c(-1.0)
    }
}
impl<const N: usize> Sub for Jet<N> {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        self + -b
    }
}
// The product rule necessarily adds derivative terms when multiplying AD values.
#[allow(clippy::suspicious_arithmetic_impl)]
impl<const N: usize> Mul for Jet<N> {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self {
            v: self.v * b.v,
            g: std::array::from_fn(|i| self.g[i] * b.v + self.v * b.g[i]),
            h: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    self.h[i][j] * b.v
                        + self.v * b.h[i][j]
                        + self.g[i] * b.g[j]
                        + b.g[i] * self.g[j]
                })
            }),
        }
    }
}
impl<const N: usize> Div for Jet<N> {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        self * b.chain(1.0 / b.v, -1.0 / b.v.powi(2), 2.0 / b.v.powi(3))
    }
}

pub fn sub<S: Scalar>(a: [S; 3], b: [S; 3]) -> [S; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn dot<S: Scalar>(a: [S; 3], b: [S; 3]) -> S {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross<S: Scalar>(a: [S; 3], b: [S; 3]) -> [S; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
