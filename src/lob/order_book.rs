use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::iter;
use crate::common::{Id, Level, Price, Qty};
use crate::structure;
use crate::structure::{Delta, MDResponse, Snapshot, Trade};
use std::ops::Mul;
use log::{debug, info, warn};
use crate::lob::order_book;
use crate::lob::order_book::DepthUpdateError::DepthStale;

type LevelsT = Vec::<Level>;

#[derive(Default, Debug)]
struct Side {
    levels: LevelsT,
}

impl Side {
    pub fn from_vec(val: Vec::<Level>) -> Self {
        Side { levels: val }
    }


    fn get_level_id(&self, price: Price) -> Option<usize> {
        if self.levels.len() < 2 {
            return None;
        }
        let tick_sz = (self.levels[1].price.clone() - self.levels[0].price.clone()).abs();

        let (idx, _lvl) =
            self.levels.iter().enumerate()
                .find(|(idx, lvl)| ((&lvl).price.clone() - price.clone()).mul(4.) < tick_sz)?;
        Some(idx)
    }

    pub fn update_diff(&mut self, lvl: Vec::<Level>, side: structure::Side, tick: &Price) -> &Self {
        let cmp = match side {
            structure::Side::Sell => { |x: &Level, y: &Level, tick: Price|
                if (x.price.clone() - y.price.clone()).same_tick(tick) {
                    Ordering::Equal
                } else if x.price.clone() > y.price.clone() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            },
            structure::Side::Buy => |x: &Level, y: &Level, tick: Price| if (x.price.clone() - y.price.clone()).same_tick(tick) {Ordering::Equal} else if (x.price.clone() > y.price.clone()) { Ordering::Greater} else { Ordering::Less },
        };
        let mut it1 = self.levels.iter().peekable();
        let mut it2 = lvl.iter().peekable();

        let mut new_levels = LevelsT::new();
        while it1.peek().is_some() && it2.peek().is_some() {
            let (l1, l2) = (it1.peek().unwrap().clone().clone(),
                            it2.peek().unwrap().clone().clone());
            match cmp(&l1, &l2, tick.clone()) {
                Ordering::Less => {
                    new_levels.push(l1.clone());
                    it1.next();
                }
                Ordering::Equal => {
                    new_levels.push(l2.clone());
                    it1.next();
                    it2.next();
                }
                Ordering::Greater => {
                    new_levels.push(l2.clone());
                    it2.next();
                }
            }
        }
        new_levels.extend(it1.map(|x| x.clone()));
        new_levels.extend(it2.map(|x| x.clone()));
        self.levels = new_levels;
        self
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum DepthUpdateError {
    DepthStale,
    StaleUpdate,
    MissedUpdate,
    WaitSnapshot,
    UnknownInstrument,
}

#[derive(Debug)]
pub struct OrderBook {
    buy: Side,
    sell: Side,
    scheduled: BTreeMap<Id, Delta>,
    snapshot_requested: bool,
    last_applied: Id,
    skip_limit: Id,
    tick: Price,
}

impl OrderBook {

    pub fn new(tick: Price) -> OrderBook {
        OrderBook { buy: Default::default(),
                    sell: Default::default(),
            scheduled: BTreeMap::new(),
            snapshot_requested: false,
            last_applied: Id(0),
            skip_limit: Id(100),
            tick,
        }
    }

    pub fn apply(&mut self, upd: structure::MDResponse) -> Result<&Self, DepthUpdateError> {
        match upd {
            MDResponse::Trade(trade) => self.apply_trade(trade),
            MDResponse::Snapshot(snapshot) => self.apply_snapshot(snapshot),
            MDResponse::Delta(delta) => {
                debug!("Id: {:?} {:?} {:?}", delta.last, delta.first, delta.last_stream);
                self.try_apply_delta(delta)
            },
            MDResponse::Ping => unreachable!(),
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) -> Result<&Self, DepthUpdateError> {
        debug!("Snapshot {:?}", snapshot.last);
        let last_id = &snapshot.last;
        self.snapshot_requested = false;

        if !self.is_stale_depth(last_id.clone()) {
            warn!("Received snapshot, although depth is not stale");
            Ok(self)
        } else {
            self.buy = Side::from_vec(snapshot.buy);
            self.sell = Side::from_vec(snapshot.sell);
            self.last_applied = snapshot.last;
            match self.scheduled.pop_first() {
                Some((k, v)) => self.try_apply_delta(v),
                None => Ok(self),
            }
        }
    }

    fn match_lvl(&mut self, lvl: Level, side: structure::Side) -> Option<(usize, Qty)> {
        let direction = match side {
            structure::Side::Buy => &self.buy,
            structure::Side::Sell => &self.sell,
        };
        let lvl_id = direction.get_level_id(lvl.price)?;
        Some((lvl_id, (&direction.levels[lvl_id].qty).clone()))
    }

    fn apply_trade(&mut self, upd: Trade) -> Result<&Self, DepthUpdateError> {
        unimplemented!()
        // if self.last_snapshot.is_none() {
        //     self.buffered.push(MDResponse::Trade(upd));
        //     Err(DepthStale)
        // } else {
        // }
    }

    fn add_diff(&mut self, delta: Delta) {
        self.sell.update_diff(delta.sell, structure::Side::Sell, &self.tick);
        self.buy.update_diff(delta.buy, structure::Side::Buy, &self.tick);
        self.last_applied = delta.last.clone();
    }

    fn match_id(&self, id: Id) -> Ordering {
        id.cmp(&(self.last_applied.to_owned()))
    }

    fn is_stale_depth(&self, id: Id) -> bool {
        self.last_applied.to_owned() + self.skip_limit.clone() < id
    }

    fn try_apply_delta(&mut self, delta: Delta) -> Result<&Self, DepthUpdateError> {
        match self.match_id(delta.last_stream.clone()) {
            Ordering::Less => {
                match self.scheduled.pop_first() {
                    Some((k, v)) => self.try_apply_delta(v),
                    None => Ok(self),
                }
                // Err(DepthUpdateError::StaleUpdate)
            },
            Ordering::Equal => {
                self.add_diff(delta);
                match self.scheduled.pop_first() {
                    Some((k, v)) => self.try_apply_delta(v),
                    None => Ok(self),
                }
                // Ok(self)
            }
            Ordering::Greater => {
                let id = delta.first.clone();
                self.scheduled.insert(id.clone(), delta);
                if self.is_stale_depth(id.clone()) {
                    if self.snapshot_requested {
                        Err(DepthUpdateError::WaitSnapshot)
                    } else {
                        self.snapshot_requested = true;
                        Err(DepthUpdateError::DepthStale)
                    }
                } else {
                    Err(DepthUpdateError::MissedUpdate)
                }
            }
        }
    }
}
