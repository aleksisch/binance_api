extern crate core;

mod common;
mod connection;
mod lob;
mod scheme;
mod structure;
mod runner;

use crate::scheme::connector::{MarketQueries, WssStream};
use crate::structure::{Instrument, MDResponse};
use clap::Parser;
use futures_util::future::join_all;
use log::{info};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use crate::lob::orderbooks::DepthBookManager;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use crate::runner::{Runner};

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

    let (tx, mut rx) = mpsc::channel(100);

    log::debug!("{:?}", &available);
    let exchanges: Vec<Arc<dyn MarketQueries + Send + Sync>> =
        vec![Arc::new(scheme::binance::Api::new())];
    let shared_exch = Arc::new(exchanges);

    let mut handles: Vec<JoinHandle<()>> = vec![];

    let insts_map: Arc<HashMap<String, Instrument>> = Arc::new(
        available
            .iter()
            .map(|x| (x.to_raw_string().clone(), x.clone()))
            .collect(),
    );

    for sz in 0..shared_exch.len() {
        for _ in 0..args.conn_num {
            let shared_vec_clone = Arc::clone(&shared_exch);
            let exch = shared_vec_clone[sz].clone();

            handles.push(Runner::create_connection(exch, tx.clone(), available.clone(), insts_map.clone()));
        }
    }

    handles.push(Runner::spawn_main_loop(tx.clone(), rx, DepthBookManager::new(available.as_ref())));

    join_all(handles).await;
    // handles.iter().map(|h| h.)
}
