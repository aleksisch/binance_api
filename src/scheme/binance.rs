use crate::common::Level;
use crate::scheme::connector::{AliasInstrument, HTTPApi, MarketQueries, Streams, WssStream};
use crate::structure::{Coin, Exchange, Feed, Instrument, MDResponse};
use crate::{common, structure};
use serde::{Deserialize, Serialize};
use std::string::ToString;
use std::time::SystemTime;

#[derive(Serialize)]
pub struct Connect {
    method: String,
    params: Vec<String>,
    id: u64,
}

#[derive(Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub baseAsset: String,
    pub quoteAsset: String,
    pub contractType: String,
    pub deliveryDate: u64,
}

#[derive(Deserialize)]
pub struct ExchangeInfo {
    symbols: Vec<Symbol>,
}

impl Connect {
    const STREAM: &'static str = "SUBSCRIBE";

    fn get_sub(inst: &Instrument, streams: &Streams) -> Vec<String> {
        streams
            .iter()
            .map(|stream| {
                let str = match stream {
                    WssStream::Trade => "aggTrade",
                    WssStream::Depth => "depth",
                };
                format!("{}@{}", inst.to_raw_string().to_lowercase(), str)
            })
            .collect()
    }

    pub fn new(id: u64, insts: &Vec<Instrument>, stream: &Streams) -> Connect {
        Connect {
            method: Self::STREAM.to_string(),
            id,
            params: insts
                .iter()
                .map(|inst| Self::get_sub(&inst, &stream))
                .flatten()
                .collect(),
        }
    }

    pub fn new_single(id: u64, inst: &Instrument, stream: &Streams) -> Connect {
        Connect {
            method: "SUBSCRIBE".parse().unwrap(),
            id,
            params: Self::get_sub(inst, &stream),
        }
    }
}

#[derive(Deserialize)]
struct Trade {
    #[serde(alias = "s")]
    symbol: String,
    #[serde(alias = "p")]
    price: String,
    #[serde(alias = "q")]
    qty: String,
    #[serde(alias = "f")]
    first_id: u64,
    #[serde(alias = "l")]
    last_id: u64,
}

impl Trade {
    fn to_regular(self, insts_map: &AliasInstrument) -> Option<structure::Trade> {
        Some(structure::Trade::new(
            insts_map.get(&self.symbol)?.clone(),
            Level::new(self.price.parse().ok()?, self.qty.parse().ok()?),
            common::Id(self.first_id),
            common::Id(self.last_id),
        ))
    }
}

#[derive(Deserialize)]
struct Delta {
    #[serde(alias = "s")]
    symbol: String,
    #[serde(alias = "U")]
    first_id: u64,
    #[serde(alias = "u")]
    last_id: u64,
    #[serde(alias = "b")]
    buy: Vec<(String, String)>,
    #[serde(alias = "a")]
    sell: Vec<(String, String)>,
}

impl Delta {
    fn to_regular(self, insts_map: &AliasInstrument) -> Option<structure::Delta> {
        let buy = self
            .buy
            .iter()
            .map(|v| Level::from_str_pair(v))
            .flatten()
            .collect();
        let sell = self
            .sell
            .iter()
            .map(|v| Level::from_str_pair(v))
            .flatten()
            .collect();
        Some(structure::Delta::new(
            insts_map.get(&self.symbol)?.clone(),
            buy,
            sell,
            common::Id(self.first_id),
            common::Id(self.last_id),
        ))
    }
}

pub struct Api;

impl Api {
    pub(crate) fn new() -> Api {
        Api {}
    }

    fn get_sub_id() -> u64 {
        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => rand::random(),
        }
    }

    pub async fn instrument_info(self) -> Vec<Instrument> {
        let res = reqwest::get("https://fapi.binance.com/fapi/v1/exchangeInfo")
            .await
            .unwrap();
        let body = res.text().await.unwrap().to_string();
        serde_json::from_str::<ExchangeInfo>(body.as_str())
            .unwrap()
            .symbols
            .iter()
            .map(|symb| {
                let feed = Feed::from_raw(&symb.contractType, symb.deliveryDate)?;
                Some(Instrument::new(
                    Coin((&symb.baseAsset).clone()),
                    Coin((&symb.quoteAsset).clone()),
                    feed,
                    Exchange::BINANCE,
                    (&symb.symbol).clone(),
                ))
            })
            .flatten()
            .collect()
    }
}

impl HTTPApi for Api {}

impl MarketQueries for Api {
    fn connect_uri(&self) -> &'static str {
        // "wss://fstream.binance.com"
        // "wss://ws-api.binance.com:443/ws-api/v3"
        "wss://fstream.binance.com/ws"
    }

    fn pong(&self) -> &'static str {
        "pong"
    }

    fn subscribe(&self, inst: &Vec<Instrument>, stream: &Streams) -> String {
        serde_json::to_string(&Connect::new(Self::get_sub_id(), &inst, &stream)).unwrap()
    }

    fn subscribe_single(&self, inst: &Instrument, stream: &Streams) -> String {
        serde_json::to_string(&Connect::new_single(Self::get_sub_id(), &inst, stream)).unwrap()
    }

    fn unsubscribe(&self, instrument: &Vec<Instrument>, stream: &Streams) -> String {
        todo!()
    }

    fn unsubscribe_single(&self, instrument: &Instrument, stream: &Streams) -> String {
        todo!()
    }

    fn handle_response(&self, resp: &String, insts_map: &AliasInstrument) -> Option<MDResponse> {
        Some(match resp.as_bytes()[6] {
            // just an optimization to avoid extra deserialization
            97 => MDResponse::Trade(
                serde_json::from_str::<Trade>(resp)
                    .ok()?
                    .to_regular(insts_map)?,
            ),
            100 => MDResponse::Delta(
                serde_json::from_str::<Delta>(resp)
                    .ok()?
                    .to_regular(insts_map)?,
            ),
            _ => MDResponse::Ping,
        })
    }
}
