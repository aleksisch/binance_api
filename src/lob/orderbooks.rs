use crate::lob::order_book::OrderBook;
use crate::structure::{Instrument, MDResponse};
use std::collections::HashMap;

struct DepthBookManager {
    books: HashMap<Instrument, OrderBook>,
}

impl DepthBookManager {
    // fn new() -> DepthBookManager {
    //     DepthBookManager { books: HashMap::new() }
    // }
    //
    // fn get(&mut self, instrument: Instrument) -> &OrderBook {
    //     self.books.entry(instrument).or_insert(OrderBook::new());
    // }
    //
    // fn update(&mut self, instrument: Instrument, response: MDResponse) -> Some(&OrderBook) {
    //     self.books.entry(instrument).or_insert(OrderBook::new());
    // }
}
