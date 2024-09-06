use crate::common::{Id, Level, Price};
use derive_new::new;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Coin(pub String);

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub enum Feed {
    FUTURE(u64),
    PERP,
    OPTION,
    SPOT,
}

impl Feed {
    pub fn from_raw(input: &str, date: u64) -> Option<Feed> {
        match input {
            "PERP" => Some(Feed::PERP),
            "OPTION" => Some(Feed::OPTION),
            "SPOT" => Some(Feed::SPOT),
            _ => Some(Feed::FUTURE(date)),
        }
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub enum Exchange {
    BINANCE,
}

#[derive(Clone, new, Eq, Hash, PartialEq)]
pub struct Instrument {
    pub base: Coin,
    pub margin: Coin,
    pub feed: Feed,
    pub exchange: Exchange,
    pub price_precision: u32,
    raw: String,
}

impl fmt::Debug for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "raw={}", self.raw)
    }
}

impl Instrument {
    pub fn to_raw_string(&self) -> &String {
        &self.raw
    }

    pub fn get_precision(&self) -> Price {
        Price::new(10_f32.powf(-1. * self.price_precision as f32))
    }
}

#[derive(new, Debug)]
pub struct Trade {
    inst: Instrument,
    info: Level,
    pub(crate) side: Side,
    first: Id,
    last: Id,
}

struct BookSide {}

#[derive(new, Debug)]
pub struct Delta {
    pub inst: Instrument,
    pub buy: Vec<Level>,
    pub sell: Vec<Level>,
    pub first: Id,
    pub last: Id,
    pub last_stream: Id,
}

#[derive(Debug, new)]
pub struct Snapshot {
    pub inst: Instrument,
    pub buy: Vec<Level>,
    pub sell: Vec<Level>,
    pub last: Id,
    pub time: u64,
}

#[derive(Debug)]
pub enum MDResponse {
    Trade(Trade),
    Snapshot(Snapshot),
    Delta(Delta),
    Ping,
}

impl MDResponse {
    pub fn get_inst(&self) -> Option<Instrument> {
        Some(match self {
            MDResponse::Ping => return None,
            MDResponse::Delta(d) => d.inst.clone(),
            MDResponse::Snapshot(d) => d.inst.clone(),
            MDResponse::Trade(d) => d.inst.clone(),
        })
    }
}
