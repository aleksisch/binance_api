use crate::common::{Id, Level};
use derive_new::new;
use std::fmt;

#[derive(Clone)]
pub struct Coin(pub String);

#[derive(Clone)]
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

#[derive(Clone)]
pub enum Exchange {
    BINANCE,
}

#[derive(Clone)]
pub struct Instrument {
    pub base: Coin,
    pub margin: Coin,
    pub feed: Feed,
    pub exchange: Exchange,
    raw: String,
}

impl fmt::Debug for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "raw={}", self.raw)
    }
}

impl Instrument {
    pub fn new(
        base: Coin,
        margin: Coin,
        feed: Feed,
        exchange: Exchange,
        raw: String,
    ) -> Instrument {
        Instrument {
            base,
            margin,
            feed,
            exchange,
            raw,
        }
    }

    pub fn to_raw_string(&self) -> &String {
        &self.raw
    }
}

#[derive(new, Debug)]
pub struct Trade {
    inst: Instrument,
    info: Level,
    first: Id,
    last: Id,
}

struct BookSide {}

#[derive(Debug)]
pub struct Snapshot(pub crate::lob::order_book::OrderBook);

#[derive(new, Debug)]
pub struct Delta {
    inst: Instrument,
    buy: Vec<Level>,
    sell: Vec<Level>,
    first: Id,
    last: Id,
}

#[derive(Debug)]
pub enum MDResponse {
    Trade(Trade),
    Snapshot(Snapshot),
    Delta(Delta),
    Ping,
}
