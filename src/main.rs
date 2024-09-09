extern crate core;

mod common;
mod config;
mod connection;
mod lob;
mod runner;
mod scheme;
mod structure;

use crate::config::MDConfig;
use crate::lob::orderbooks::DepthBookManager;
use crate::runner::Runner;
use crate::scheme::connector::{HTTPApi, MarketQueries};
use crate::structure::{Exchange, Instrument};
use clap::Parser;
use futures_util::future;
use futures_util::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Translator from assembly to binary
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_values_t = ["BTCUSDT".to_string()])]
    instruments: Vec<String>,
    #[arg(short, long, default_value = "3")]
    num_conn: u32,
    #[arg(short, long, default_value = "src/endpoints.toml")]
    config_path: String,
    #[arg(
        long,
        default_value = "100",
        help = "Maximum number of missed ids, after which request snapshot"
    )]
    delay_limit: u32,
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let args = Args::parse();
    log::info!(
        "Passed arguments: {:?} {:?}",
        args.instruments,
        args.num_conn
    );
    let cfg = MDConfig::new(args.config_path).expect("Failed to parse");
    let binance_cfg = cfg.get(Exchange::BINANCE).expect("Expected binance config");

    let (tx, rx) = mpsc::channel(100);
    let wss_exchanges: Arc<Vec<Arc<dyn MarketQueries + Send + Sync>>> = Arc::new(vec![Arc::new(
        scheme::binance::Api::new(binance_cfg.clone()),
    )]);

    // todo: replace vec with HashMap. It's not easy due to async trait
    let http_exchanges: Vec<(Exchange, Box<dyn HTTPApi + Send + Sync>)> = vec![(
        Exchange::BINANCE,
        Box::new(scheme::binance::Api::new(binance_cfg.clone())),
    )];

    let available: Arc<Vec<Instrument>> = Arc::new(
        future::join_all(
            http_exchanges
                .iter()
                .map(|(_, exch)| async {
                    exch.instrument_info()
                        .await
                        .into_iter()
                        .filter(|inst| args.instruments.contains(inst.to_raw_string()))
                        .into_iter()
                        .collect::<Vec<Instrument>>()
                })
                .collect::<Vec<_>>(),
        )
        .await
        .into_iter()
        .flatten()
        .collect(),
    );
    log::debug!("{:?}", &available);

    let mut handles: Vec<JoinHandle<()>> = vec![];

    let insts_map: Arc<HashMap<String, Instrument>> = Arc::new(
        available
            .iter()
            .map(|x| (x.to_raw_string().clone(), x.clone()))
            .collect(),
    );

    for sz in 0..wss_exchanges.len() {
        for _ in 0..args.num_conn {
            let shared_vec_clone = Arc::clone(&wss_exchanges);
            let exch = shared_vec_clone[sz].clone();

            handles.push(Runner::create_connection(
                exch,
                tx.clone(),
                available.clone(),
                insts_map.clone(),
            ));
        }
    }

    handles.push(Runner::spawn_main_loop(
        http_exchanges,
        tx.clone(),
        rx,
        DepthBookManager::new(available.as_ref()),
    ));

    join_all(handles).await;
}
