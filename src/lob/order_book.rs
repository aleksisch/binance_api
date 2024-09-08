use crate::common::{Id, Level, Precision, Price, Qty};
use crate::structure;
use crate::structure::{Delta, MDResponse, Snapshot, Trade};
use log::{debug, info, warn};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

type LevelsT = Vec<Level>;

#[derive(Default, Debug)]
struct Side {
    pub levels: LevelsT,
    limit: usize,
}

impl Side {
    pub fn from_vec(levels: Vec<Level>, limit: usize) -> Self {
        Side { levels, limit }
    }

    fn get_level_id(&self, price: Price, tick_sz: &Precision) -> Option<usize> {
        if self.levels.len() < 2 {
            return None;
        }

        let (idx, _lvl) =
            self.levels.iter().enumerate().find(|(_idx, lvl)| {
                ((&lvl).price.clone() - price.clone()).same_tick(&tick_sz.price)
            })?;
        Some(idx)
    }

    fn push_level(lvls: &mut LevelsT, lvl: Level, tick: &Qty) -> bool {
        let need_insert = !lvl.qty.clone().same_tick(tick.clone());
        if need_insert {
            lvls.push(lvl);
        }
        need_insert
    }

    pub fn update_diff(
        &mut self,
        lvl: Vec<Level>,
        side: structure::Side,
        prec: &Precision,
    ) -> &Self {
        let (x_more_y, y_more_x) = match side {
            structure::Side::Buy => (Ordering::Less, Ordering::Greater),
            structure::Side::Sell => (Ordering::Greater, Ordering::Less),
        };
        let cmp = |x: &Level, y: &Level, tick: &Price| {
            if (x.price.clone() - y.price.clone()).same_tick(tick) {
                Ordering::Equal
            } else if x.price.clone() > y.price.clone() {
                x_more_y
            } else {
                y_more_x
            }
        };
        let mut it1 = self.levels.iter().peekable();
        let mut it2 = lvl.iter().peekable();

        let mut new_levels = LevelsT::new();
        while it1.peek().is_some() && it2.peek().is_some() {
            let (l1, l2) = (
                (*it1.peek().unwrap()).clone(),
                (*it2.peek().unwrap()).clone(),
            );
            match cmp(&l1, &l2, &prec.price) {
                Ordering::Less => {
                    Self::push_level(&mut new_levels, l1, &prec.qty);
                    it1.next();
                }
                Ordering::Equal => {
                    Self::push_level(&mut new_levels, l2, &prec.qty);
                    it1.next();
                    it2.next();
                }
                Ordering::Greater => {
                    Self::push_level(&mut new_levels, l2, &prec.qty);
                    it2.next();
                }
            }
        }
        new_levels.extend(it1.map(|x| x.clone()));
        new_levels.extend(it2.map(|x| x.clone()));
        new_levels.truncate(self.limit);
        self.levels = new_levels;
        self
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum DepthUpdateError {
    DepthStale,
    MissedUpdate,
    WaitSnapshot,
    StaleUpdate,
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
    depth_limit: usize,
    precision: Precision,
}

impl Display for OrderBook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "depthbook updated: \n")?;
        for lvl in self.sell.levels.iter().rev() {
            write!(f, "{} - {}\n", lvl.price, lvl.qty)?;
        }
        write!(f, "=======================\n")?;
        for lvl in &self.buy.levels {
            write!(f, "{} - {}\n", lvl.price, lvl.qty)?;
        }
        Ok(())
    }
}

impl OrderBook {
    pub fn new(precision: Precision, depth_limit: usize) -> OrderBook {
        OrderBook {
            buy: Default::default(),
            sell: Default::default(),
            scheduled: BTreeMap::new(),
            snapshot_requested: false,
            last_applied: Id(0),
            skip_limit: Id(100),
            depth_limit,
            precision,
        }
    }

    pub fn apply(&mut self, upd: structure::MDResponse) -> Result<&Self, DepthUpdateError> {
        match upd {
            MDResponse::Trade(trade) => self.apply_trade(trade),
            MDResponse::Snapshot(snapshot) => self.apply_snapshot(snapshot),
            MDResponse::Delta(delta) => {
                debug!(
                    "Id: {:?} {:?} {:?}",
                    delta.last, delta.first, delta.last_stream
                );
                self.scheduled.insert(delta.first.clone(), delta);
                self.try_apply_scheduled()
            }
            MDResponse::Ping => unreachable!(),
        }
    }

    fn find_first_id(snap_id: Id, events: &BTreeMap<Id, Delta>) -> Option<Id> {
        for (k, v) in events {
            if v.last < snap_id {
                continue;
            } else if k <= &snap_id {
                return Some(v.last_stream.clone());
            } else {
                warn!("No event associated with snapshot");
                return None;
            }
        }
        None
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) -> Result<&Self, DepthUpdateError> {
        debug!("Snapshot {:?}", snapshot.last);
        let last_id = &snapshot.last;
        self.snapshot_requested = false;

        if !self.is_stale_depth(last_id.clone()) {
            warn!("Received snapshot, although depth is not stale");
            return Ok(self);
        }
        self.buy = Side::from_vec(snapshot.buy, self.depth_limit);
        self.sell = Side::from_vec(snapshot.sell, self.depth_limit);
        match Self::find_first_id(snapshot.last, &self.scheduled) {
            Some(x) => {
                self.last_applied = x;
                self.try_apply_scheduled()
            }
            None => {
                self.snapshot_requested = true;
                info!("Rerequest snapshot info");
                Err(DepthUpdateError::DepthStale)
            }
        }
    }

    fn match_lvl(&mut self, lvl: Level, side: structure::Side) -> Option<(usize, Qty)> {
        let direction = match side {
            structure::Side::Buy => &self.buy,
            structure::Side::Sell => &self.sell,
        };
        let lvl_id = direction.get_level_id(lvl.price, &self.precision)?;
        Some((lvl_id, (&direction.levels[lvl_id].qty).clone()))
    }

    fn apply_trade(&mut self, _upd: Trade) -> Result<&Self, DepthUpdateError> {
        unimplemented!()
        // if self.last_snapshot.is_none() {
        //     self.buffered.push(MDResponse::Trade(upd));
        //     Err(DepthStale)
        // } else {
        // }
    }

    fn add_diff(&mut self, delta: Delta) {
        self.sell
            .update_diff(delta.sell, structure::Side::Sell, &self.precision);
        self.buy
            .update_diff(delta.buy, structure::Side::Buy, &self.precision);
        self.last_applied = delta.last.clone();
    }

    fn match_id(&self, id: Id) -> Ordering {
        id.cmp(&(self.last_applied.to_owned()))
    }

    fn is_stale_depth(&self, id: Id) -> bool {
        self.last_applied.to_owned() + self.skip_limit.clone() < id
    }

    fn try_apply_scheduled(&mut self) -> Result<&Self, DepthUpdateError> {
        let mut error: Option<DepthUpdateError> = None;

        while let Some((_id, v)) = self.scheduled.pop_first() {
            let res = self.try_apply_delta(v);
            match res {
                Some(DepthUpdateError::StaleUpdate) => error = Some(DepthUpdateError::StaleUpdate),
                Some(err) => return Err(err),
                None => error = None,
            }
        }
        match error {
            None => Ok(self),
            Some(err) => Err(err),
        }
    }

    fn try_apply_delta(&mut self, delta: Delta) -> Option<DepthUpdateError> {
        match self.match_id(delta.last_stream.clone()) {
            Ordering::Less => Some(DepthUpdateError::StaleUpdate),
            Ordering::Equal => {
                self.add_diff(delta);
                None
            }
            Ordering::Greater => {
                let id = delta.first.clone();
                self.scheduled.insert(id.clone(), delta);
                Some(if self.is_stale_depth(id.clone()) {
                    if self.snapshot_requested {
                        DepthUpdateError::WaitSnapshot
                    } else {
                        self.snapshot_requested = true;
                        DepthUpdateError::DepthStale
                    }
                } else {
                    DepthUpdateError::MissedUpdate
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::common::{Id, Level, Precision, Price, Qty};
    use crate::lob::order_book::{DepthUpdateError, OrderBook, Side};
    use crate::structure;
    use crate::structure::{Coin, Delta, Exchange, Feed, Instrument, MDResponse, Snapshot};
    use std::iter::zip;

    fn any_inst(tick_sz: f32) -> Instrument {
        Instrument::new(
            Coin("BTC".into()),
            Coin("USD".into()),
            Feed::PERP,
            Exchange::BINANCE,
            gen_prec(tick_sz, tick_sz),
            "BTCUSD".into(),
        )
    }

    fn gen_prec(price: f32, qty: f32) -> Precision {
        Precision::new(Price(price), Qty(qty))
    }

    fn compare_lvls(lvl1: &Vec<Level>, lvl2: &Vec<Level>, precision: &Precision) {
        for (v1, v2) in zip(lvl1, lvl2) {
            assert!(v1.eq(v2.clone(), precision));
        }
    }

    fn compare(depth: &OrderBook, buy: Vec<Level>, sell: Vec<Level>, precision: &Precision) {
        compare_lvls(&buy, &depth.buy.levels, &precision);
        compare_lvls(&sell, &depth.sell.levels, &precision);
    }

    #[test]
    fn update_diff() {
        let mut buy_prev = Side::from_vec(
            vec![
                Level::from_float_pair(10., 10.),
                Level::from_float_pair(11., 10.),
                Level::from_float_pair(12., 5.),
            ],
            3,
        );

        let buy_new = vec![
            Level::from_float_pair(11., 5.),
            Level::from_float_pair(12., 0.),
            Level::from_float_pair(13., 6.),
        ];

        let tick = 0.01;
        let precision = Precision::new(Price::new(tick), Qty::new(tick));
        buy_prev.update_diff(buy_new, structure::Side::Sell, &precision);
        let mut it = buy_prev.levels.iter();
        assert!(it
            .next()
            .unwrap()
            .eq(Level::from_float_pair(10., 10.), &precision));
        assert!(it
            .next()
            .unwrap()
            .eq(Level::from_float_pair(11., 5.), &precision));
        assert!(it
            .next()
            .unwrap()
            .eq(Level::from_float_pair(13., 6.), &precision));
        assert!(it.next().is_none());

        let mut sell_prev = Side::from_vec(
            vec![
                Level::from_float_pair(12., 5.),
                Level::from_float_pair(11., 10.),
                Level::from_float_pair(10., 10.),
            ],
            3,
        );
        let sell_new = vec![
            Level::from_float_pair(13., 6.),
            Level::from_float_pair(12., 0.),
            Level::from_float_pair(11., 5.),
        ];
        sell_prev.update_diff(sell_new, structure::Side::Buy, &precision);
        let mut it = sell_prev.levels.iter();
        assert!(it
            .next()
            .unwrap()
            .eq(Level::from_float_pair(13., 6.), &precision));
        assert!(it
            .next()
            .unwrap()
            .eq(Level::from_float_pair(11., 5.), &precision));
        assert!(it
            .next()
            .unwrap()
            .eq(Level::from_float_pair(10., 10.), &precision));
        assert!(it.next().is_none());
    }

    #[test]
    fn zero_qty() {
        let levels = vec![
            Level::from_float_pair(10., 10.),
            Level::from_float_pair(11., 10.),
            Level::from_float_pair(12., 5.),
        ];
        let mut buy_prev = Side::from_vec(levels.clone(), 3);
        let prec = gen_prec(0.01, 0.01);
        buy_prev.update_diff(
            vec![Level::from_float_pair(9., 0.)],
            structure::Side::Sell,
            &prec,
        );
        compare_lvls(&buy_prev.levels, &levels, &prec)
    }

    #[test]
    fn simple_update() {
        const TICK_SZ: f32 = 0.01;
        let inst: Instrument = any_inst(TICK_SZ);
        const FINAL_SZ: usize = 4;

        let mut book = OrderBook::new(inst.precision.clone(), FINAL_SZ);

        let buy_prev = vec![
            Level::from_float_pair(10., 10.),
            Level::from_float_pair(11., 10.),
        ];
        let sell_prev = vec![
            Level::from_float_pair(9.99, 100.),
            Level::from_float_pair(9.98, 100.),
        ];
        let delta1 = MDResponse::Delta(Delta::new(
            inst.clone(),
            buy_prev,
            sell_prev,
            book.skip_limit.clone() + Id(99),
            book.skip_limit.clone() + Id(101),
            book.skip_limit.clone() + Id(98),
        ));

        let buy_post = vec![
            Level::from_float_pair(101., 5.),
            Level::from_float_pair(102., 10.),
        ];
        let sell_post = vec![
            Level::from_float_pair(99.99, 10.),
            Level::from_float_pair(99.98, 10.),
        ];
        let delta2 = MDResponse::Delta(Delta::new(
            inst.clone(),
            buy_post.clone(),
            sell_post.clone(),
            book.skip_limit.clone() + Id(102),
            book.skip_limit.clone() + Id(110),
            book.skip_limit.clone() + Id(101),
        ));

        let buy = vec![
            Level::from_float_pair(100., 100.),
            Level::from_float_pair(101., 10.),
        ];
        let sell = vec![
            Level::from_float_pair(99.99, 100.),
            Level::from_float_pair(99.98, 100.),
        ];
        let false_snapshot = MDResponse::Snapshot(Snapshot::new(
            inst.clone(),
            buy.clone(),
            sell.clone(),
            book.skip_limit.clone() + Id(100),
            0,
        ));
        let snapshot = MDResponse::Snapshot(Snapshot::new(
            inst.clone(),
            buy.clone(),
            sell.clone(),
            book.skip_limit.clone() + Id(102),
            0,
        ));

        assert_eq!(book.apply(delta1).err(), Some(DepthUpdateError::DepthStale)); // request snapshot
        assert_eq!(
            book.apply(delta2).err(),
            Some(DepthUpdateError::WaitSnapshot)
        ); // wait snapshot
        compare(
            book.apply(snapshot).unwrap(),
            Side::from_vec(buy, FINAL_SZ + 1)
                .update_diff(buy_post, structure::Side::Buy, &inst.precision)
                .levels
                .clone(),
            Side::from_vec(sell, FINAL_SZ + 1)
                .update_diff(sell_post, structure::Side::Sell, &inst.precision)
                .levels
                .clone(),
            &inst.precision,
        ); // receive snapshot, apply updates
    }
}
