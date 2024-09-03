use crate::common::Level;
use crate::structure;
use crate::structure::{Delta, MDResponse, Snapshot, Trade};

#[derive(Default, Debug)]
struct Side {
    levels: [Level; 20],
}

#[derive(Default, Debug)]
pub(crate) struct OrderBook {
    buy: Side,
    sell: Side,
}

impl OrderBook {
    fn apply(&mut self, upd: structure::MDResponse) {
        match upd {
            MDResponse::Trade(trade) => self.apply_trade(trade),
            MDResponse::Snapshot(snapshot) => self.apply_snapshot(snapshot),
            MDResponse::Delta(delta) => self.apply_delta(delta),
            MDResponse::Ping => unreachable!(),
        }
    }

    fn apply_trade(&mut self, upd: Trade) {}

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        *self = snapshot.0;
    }

    fn apply_delta(&self, delta: Delta) {}
}
