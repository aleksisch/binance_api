use crate::lob::order_book::{OrderBook, DepthUpdateError};
use crate::structure::{Instrument, MDResponse};
use std::collections::HashMap;
use log::{error, info};
use crate::lob::order_book::DepthUpdateError::UnknownInstrument;

pub struct DepthBookManager {
    books: HashMap<Instrument, OrderBook>,
}

impl DepthBookManager {
    pub fn new(insts: &Vec::<Instrument>) -> DepthBookManager {
        DepthBookManager { books: insts.iter().map(|inst| (inst.clone(), OrderBook::new(inst.get_precision()))).collect() }
    }

    pub fn update(&mut self, instrument: &Instrument, response: MDResponse) -> Result<&OrderBook, DepthUpdateError> {
        match self.books.get_mut(instrument) {
            None => Err(UnknownInstrument),
            Some(book) => book.apply(response),
        }
    }
}
