use derive_new::new;
use std::num::ParseFloatError;
use std::str::FromStr;
use derive_more::{Add, Mul, Sub};


pub const DEPTHBOOK_DEPTH: usize = 20;

#[derive(Default, Debug, Clone, PartialEq, PartialOrd, Sub, Mul, new)]
pub struct Price(f32);

impl Price {
    pub fn abs(self) -> Self {
        Price(self.0.abs())
    }

    pub fn same_tick(self, tick: Price) -> bool {
        self.abs() * 4. < tick
    }
}

#[derive(Default, Debug, Clone, Sub, Mul, PartialEq, PartialOrd, new)]
pub struct Qty(f32);

impl Qty {
    pub fn abs(self) -> Self {
        Qty(self.0.abs())
    }
}

impl FromStr for Qty {
    type Err = ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Qty::new(s.parse()?))
    }
}
impl FromStr for Price {
    type Err = ParseFloatError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Price::new(s.parse()?))
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Add)]
pub struct Id(pub u64);

#[derive(Default, Debug, Clone, new)]
pub struct Level {
    pub price: Price,
    pub qty: Qty,
}

impl Level {
    pub fn from_str_pair((p, q): &(String, String)) -> Option<Self> {
        Some(Level::new(p.parse().ok()?, q.parse().ok()?))
    }

    pub fn from_float_pair(p: f32, q: f32) -> Self {
        Level::new(Price::new(p.clone()), Qty::new(q.clone()))
    }

    pub fn eq(&self, rhs: Level, tick_price: Price, qty_precision: Qty) -> bool {
        (rhs.price - self.price.clone()).same_tick(tick_price) &&
            (rhs.qty - self.qty.clone()).abs() * 4. < qty_precision
    }
}
