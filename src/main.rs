extern crate core;

mod common;
mod connection;
mod lob;
mod scheme;
mod structure;

use crate::scheme::connector::{MarketQueries, WssStream};
use crate::structure::{Instrument, MDResponse};
use clap::Parser;
use futures_util::future::join_all;
use log::{info};
use std::collections::HashMap;
use std::sync::Arc;

/// Translator from assembly to binary
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Assembly file
    #[arg(short, long, default_values_t = ["BNBUSDT".to_string()])]
    instruments: Vec<String>,
    #[arg(short, long, default_value = "1")]
    conn_num: u32,
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let args = Args::parse();
    log::info!(
        "Passed arguments: {:?} {:?}",
        args.instruments,
        args.conn_num
    );
    let available: Arc<Vec<Instrument>> = Arc::new(
        scheme::binance::Api::new()
            .instrument_info()
            .await
            .into_iter()
            .filter(|inst| args.instruments.contains(inst.to_raw_string()))
            .collect(),
    );
    log::debug!("{:?}", &available);
    let exchanges: Vec<Box<dyn MarketQueries + Send + Sync>> =
        vec![Box::new(scheme::binance::Api::new())];
    let shared_exch = Arc::new(exchanges);

    let streams = vec![WssStream::Depth, WssStream::Trade];

    let mut handles = vec![];

    let insts_map: Arc<HashMap<String, Instrument>> = Arc::new(
        available
            .iter()
            .map(|x| (x.to_raw_string().clone(), x.clone()))
            .collect(),
    );

    for sz in 0..shared_exch.len() {
        for _ in 0..args.conn_num {
            let shared_vec_clone = Arc::clone(&shared_exch);
            let insts_clone = Arc::clone(&available);
            let insts_map_clone = Arc::clone(&insts_map);
            let streams_clone = streams.clone();
            handles.push(tokio::spawn(async move {
                let exch = &shared_vec_clone[sz];
                let mut client = crate::connection::WsClient::connect_to(&exch.connect_uri()).await;
                client
                    .send((&exch).subscribe(insts_clone.as_ref(), &streams_clone))
                    .await;
                loop {
                    let data = client.wait().await;
                    match data {
                        None => break,
                        Some(res) => {
                            let result = exch.handle_response(&res, insts_map_clone.as_ref());
                            if result.is_none() {
                                info!("Couldn't parse {}", res);
                                continue;
                            }
                            info!("Parsed: {:?}", result);
                            match result.unwrap() {
                                MDResponse::Trade(tr) => {}
                                MDResponse::Snapshot(snap) => {}
                                MDResponse::Delta(delta) => {}
                                MDResponse::Ping => {}
                            }
                        }
                    }
                }
            }));
        }
    }
    join_all(handles).await;
    // handles.iter().map(|h| h.)
}
