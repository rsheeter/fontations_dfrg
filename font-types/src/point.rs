use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Two dimensional point with a generic coordinate type.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Point<T> {
    /// X coordinate.
    pub x: T,
    /// Y coordinate.
    pub y: T,
}

impl<T> Point<T> {
    /// Creates a new point with the given x and y coordinates.
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    /// Creates a new point from a single value assigned to both coordinates.
    pub const fn broadcast(xy: T) -> Self
    where
        T: Copy,
    {
        Self { x: xy, y: xy }
    }

    /// Maps `Point<T>` to `Point<U>` by applying a function to each coordinate.
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Point<U> {
        Point {
            x: f(self.x),
            y: f(self.y),
        }
    }
}

impl<T> Add for Point<T>
where
    T: Add<Output = T>,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T> AddAssign for Point<T>
where
    T: AddAssign,
{
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T> Sub for Point<T>
where
    T: Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl<T> SubAssign for Point<T>
where
    T: SubAssign,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl<T> Mul for Point<T>
where
    T: Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl<T> Mul<T> for Point<T>
where
    T: Mul<Output = T> + Copy,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl<T> MulAssign for Point<T>
where
    T: MulAssign,
{
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
    }
}

impl<T> MulAssign<T> for Point<T>
where
    T: MulAssign + Copy,
{
    fn mul_assign(&mut self, rhs: T) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl<T> Div for Point<T>
where
    T: Div<Output = T>,
{
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl<T> Div<T> for Point<T>
where
    T: Div<Output = T> + Copy,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl<T> DivAssign for Point<T>
where
    T: DivAssign,
{
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
    }
}

impl<T> DivAssign<T> for Point<T>
where
    T: DivAssign + Copy,
{
    fn div_assign(&mut self, rhs: T) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl<T> Neg for Point<T>
where
    T: Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Point;
    use crate::F26Dot6;

    #[test]
    fn map() {
        assert_eq!(
            Point::new(42.5, 20.1).map(F26Dot6::from_f64),
            Point::new(F26Dot6::from_f64(42.5), F26Dot6::from_f64(20.1))
        );
    }

    #[test]
    fn add() {
        assert_eq!(Point::new(1, 2) + Point::new(3, 4), Point::new(4, 6));
        let mut point = Point::new(1, 2);
        point += Point::new(3, 4);
        assert_eq!(point, Point::new(4, 6));
    }

    #[test]
    fn sub() {
        assert_eq!(Point::new(1, 2) - Point::new(3, 4), Point::new(-2, -2));
        let mut point = Point::new(1, 2);
        point -= Point::new(3, 4);
        assert_eq!(point, Point::new(-2, -2));
    }

    #[test]
    fn mul() {
        assert_eq!(Point::new(1, 2) * Point::new(3, 4), Point::new(3, 8));
        let mut point = Point::new(1, 2);
        point *= Point::new(3, 4);
        assert_eq!(point, Point::new(3, 8));
        assert_eq!(Point::new(1, 2) * 8, Point::new(8, 16));
    }

    #[test]
    fn div() {
        assert_eq!(Point::new(10, 16) / Point::new(2, 3), Point::new(5, 5));
        let mut point = Point::new(10, 16);
        point /= Point::new(2, 3);
        assert_eq!(point, Point::new(5, 5));
        assert_eq!(Point::new(10, 16) / 2, Point::new(5, 8));
    }

    #[test]
    fn neg() {
        assert_eq!(-Point::new(1, -2), Point::new(-1, 2));
    }
}
