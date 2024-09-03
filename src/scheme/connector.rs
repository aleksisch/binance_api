use crate::structure::{Instrument, MDResponse};
use std::collections::HashMap;

#[derive(Clone)]
pub enum WssStream {
    Trade,
    Depth,
}

pub trait HTTPApi {
    // todo: generalize http calls with this trait
    // fn instrument_info(self) -> Vec::<Instrument>;
}

pub type Streams = Vec<WssStream>;
pub type Instruments = Vec<Instrument>;
pub type AliasInstrument = HashMap<String, Instrument>;
pub trait MarketQueries {
    fn connect_uri(&self) -> &'static str;
    fn pong(&self) -> &'static str;
    fn subscribe(&self, inst: &Instruments, stream: &Streams) -> String;
    fn subscribe_single(&self, inst: &Instrument, stream: &Streams) -> String;
    fn unsubscribe(&self, instrument: &Instruments, stream: &Streams) -> String;
    fn unsubscribe_single(&self, instrument: &Instrument, stream: &Streams) -> String;
    fn handle_response(&self, resp: &String, inst_map: &AliasInstrument) -> Option<MDResponse>;
}
