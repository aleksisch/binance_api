use derive_new::new;
use std::num::ParseFloatError;
use std::str::FromStr;

#[derive(Default, Debug, new)]
pub struct Price(f32);

#[derive(Default, Debug, new)]
pub struct Qty(f32);

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

#[derive(Default, Debug)]
pub struct Id(pub u64);

#[derive(Default, Debug, new)]
pub struct Level {
    price: Price,
    qty: Qty,
}

impl Level {
    pub fn from_str_pair((p, q): &(String, String)) -> Option<Self> {
        Some(Level::new(p.parse().ok()?, q.parse().ok()?))
    }
}
