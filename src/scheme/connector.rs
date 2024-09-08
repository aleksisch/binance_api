use crate::structure;
use crate::structure::{Instrument, MDResponse};
use async_trait::async_trait;
use std::collections::HashMap;

#[derive(Clone)]
pub enum WssStream {
    Trade,
    Depth,
}

#[async_trait]
pub trait HTTPApi {
    // todo: generalize http calls with this trait
    async fn instrument_info(&self) -> Vec<Instrument>;
    async fn request_depth_shapshot(&self, inst: Instrument) -> structure::Snapshot;
}

pub type Streams = Vec<WssStream>;
pub type Instruments = Vec<Instrument>;
pub type AliasInstrument = HashMap<String, Instrument>;
pub trait MarketQueries {
    fn connect_uri(&self) -> &String;
    fn pong(&self) -> &'static str;
    fn subscribe(&self, inst: &Instruments, stream: &Streams) -> String;
    fn subscribe_single(&self, inst: &Instrument, stream: &Streams) -> String;
    fn unsubscribe(&self, instrument: &Instruments, stream: &Streams) -> String;
    fn unsubscribe_single(&self, instrument: &Instrument, stream: &Streams) -> String;
    fn handle_response(&self, resp: &String, inst_map: &AliasInstrument) -> Option<MDResponse>;
}
