use crate::common::{Level, Precision, Price, Qty};
use crate::config::ExchangeConfig;
use crate::scheme::connector::{AliasInstrument, HTTPApi, MarketQueries, Streams, WssStream};
use crate::scheme::http_client::HTTPClient;
use crate::structure::{Coin, Exchange, Feed, Instrument, MDResponse, Side};
use crate::{common, structure};
use async_trait::async_trait;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::string::ToString;
use std::time::SystemTime;

#[derive(Serialize)]
pub struct Connect {
    method: String,
    params: Vec<String>,
    id: u64,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
pub struct Symbol {
    pub symbol: String,
    pub baseAsset: String,
    pub quoteAsset: String,
    pub contractType: String,
    pub filters: Vec<Value>,
    pub deliveryDate: u64,
}

#[derive(Deserialize)]
pub struct ExchangeInfo {
    symbols: Vec<Symbol>,
}

impl Symbol {
    pub fn find_filter(filters: &Vec<Value>, name: &str) -> Option<Value> {
        for filter in filters {
            let obj = filter.as_object()?;
            if obj.get("filterType")?.as_str() == name.into() {
                return Some(filter.clone());
            }
        }
        None
    }

    pub fn get_precision(&self) -> Precision {
        let price =
            Self::find_filter(&self.filters, "PRICE_FILTER").expect("Price filter not found");
        let qty = Self::find_filter(&self.filters, "LOT_SIZE").expect("Qty filter not found");
        Precision::new(
            Price::new(
                price
                    .as_object()
                    .expect("Filter expected to be object")
                    .get("tickSize")
                    .expect("tickSize not found")
                    .as_str()
                    .unwrap()
                    .parse::<f32>()
                    .unwrap(),
            ),
            Qty::new(
                qty.as_object()
                    .expect("Filter expected to be object")
                    .get("stepSize")
                    .expect("stepSize not found")
                    .as_str()
                    .unwrap()
                    .parse::<f32>()
                    .unwrap(),
            ),
        )
    }
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
    #[serde(alias = "m")]
    is_mm: bool,
}

impl Trade {
    fn to_regular(self, insts_map: &AliasInstrument) -> Option<structure::Trade> {
        Some(structure::Trade::new(
            insts_map.get(&self.symbol)?.clone(),
            Level::new(self.price.parse().ok()?, self.qty.parse().ok()?),
            if self.is_mm == true {
                Side::Sell
            } else {
                Side::Buy
            },
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
    #[serde(alias = "pu")]
    last_stream: u64,
    #[serde(alias = "b")]
    buy: Vec<(String, String)>,
    #[serde(alias = "a")]
    sell: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct Snapshot {
    #[serde(alias = "E")]
    message_time: u64,
    #[serde(alias = "lastUpdateId")]
    last_id: u64,
    #[serde(alias = "bids")]
    buy: Vec<(String, String)>,
    #[serde(alias = "asks")]
    sell: Vec<(String, String)>,
}

fn pair_to_levels<'a, I>(pairs: I) -> Vec<Level>
where
    I: Iterator<Item = &'a (String, String)>,
{
    pairs.map(|v| Level::from_str_pair(v)).flatten().collect()
}

impl Delta {
    fn to_regular(self, insts_map: &AliasInstrument) -> Option<structure::Delta> {
        Some(structure::Delta::new(
            insts_map.get(&self.symbol)?.clone(),
            pair_to_levels(self.buy.iter().rev()),
            pair_to_levels(self.sell.iter()),
            common::Id(self.first_id),
            common::Id(self.last_id),
            common::Id(self.last_stream),
        ))
    }
}

impl Snapshot {
    fn to_regular(self, inst: Instrument) -> Option<structure::Snapshot> {
        Some(structure::Snapshot::new(
            inst,
            pair_to_levels(self.buy.iter().rev()),
            pair_to_levels(self.sell.iter()),
            common::Id(self.last_id),
            self.message_time,
        ))
    }
}

pub struct Api {
    cfg: ExchangeConfig,
}

impl Api {
    fn get_api_url(&self, s: &str) -> String {
        self.cfg.http_api.to_owned() + s
    }

    pub(crate) fn new(cfg: ExchangeConfig) -> Api {
        Api { cfg }
    }

    fn get_sub_id() -> u64 {
        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => rand::random(),
        }
    }
}

#[async_trait]
impl HTTPApi for Api {
    async fn instrument_info(&self) -> Vec<Instrument> {
        HTTPClient::get::<ExchangeInfo>(self.get_api_url(self.cfg.exchange_info.as_ref()).as_ref())
            .await
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
                    symb.get_precision(),
                    (&symb.symbol).clone(),
                ))
            })
            .flatten()
            .collect()
    }

    async fn request_depth_shapshot(&self, inst: Instrument) -> structure::Snapshot {
        HTTPClient::get::<Snapshot>(
            Url::parse_with_params(
                &self.get_api_url(self.cfg.snapshot.as_ref()),
                &[("symbol", inst.to_raw_string())],
            )
            .unwrap()
            .as_str(),
        )
        .await
        .unwrap()
        .to_regular(inst)
        .unwrap()
    }
}

impl MarketQueries for Api {
    fn connect_uri(&self) -> &String {
        &self.cfg.wss_api
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

    fn unsubscribe(&self, _instrument: &Vec<Instrument>, _stream: &Streams) -> String {
        todo!()
    }

    fn unsubscribe_single(&self, _instrument: &Instrument, _stream: &Streams) -> String {
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
            _ => {
                if resp == "ping" {
                    MDResponse::Ping
                } else {
                    return None;
                }
            }
        })
    }
}
