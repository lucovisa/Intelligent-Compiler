//! Минимальная математика для 2D/3D. Позже заменим/дополним SIMD-версией.
use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
}

macro_rules! impl_ops {
    ($t:ident { $($f:ident),+ }) => {
        impl Add for $t { type Output = Self; fn add(self, o: Self) -> Self { Self { $($f: self.$f + o.$f),+ } } }
        impl Sub for $t { type Output = Self; fn sub(self, o: Self) -> Self { Self { $($f: self.$f - o.$f),+ } } }
        impl Mul<f32> for $t { type Output = Self; fn mul(self, k: f32) -> Self { Self { $($f: self.$f * k),+ } } }
        impl AddAssign for $t { fn add_assign(&mut self, o: Self) { $(self.$f += o.$f;)+ } }
    };
}
impl_ops!(Vec2 { x, y });
impl_ops!(Vec3 { x, y, z });

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cross_is_orthogonal() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(a.cross(b), Vec3::new(0.0, 0.0, 1.0));
    }
    #[test]
    fn vec2_ops() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v.length(), 5.0);
        assert_eq!(v * 2.0, Vec2::new(6.0, 8.0));
    }
}
