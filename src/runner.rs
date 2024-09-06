use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use log::{error, info, warn};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use crate::lob::order_book::DepthUpdateError;
use crate::lob::orderbooks::DepthBookManager;
use crate::scheme;
use crate::scheme::connector::{MarketQueries, WssStream};
use crate::structure::{Instrument, MDResponse, Snapshot};


pub struct Runner;

impl Runner {
    pub async fn request_snapshot(inst: &Instrument) -> Snapshot {
        let raw = inst.to_raw_string();

        info!("Request depthbook for {}", raw);
        let resp = scheme::binance::Api::request_depth_shapshot(inst.clone()).await;
        info!("Got response for {} {:?}", raw, &resp);
        resp
    }

    fn get_streams() -> Vec<WssStream> {
        vec![WssStream::Depth, WssStream::Trade]
    }

    pub fn create_connection(exch: Arc<dyn MarketQueries + Send + Sync>,
                                   sender: Sender<MDResponse>,
                                   insts: Arc<Vec<Instrument>>,
                                   insts_map: Arc<HashMap<String, Instrument>>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut client = crate::connection::WsClient::connect_to(&exch.connect_uri()).await;
            client
                .send((&exch).subscribe(insts.as_ref(), &Self::get_streams()))
                .await;
            loop {
                let data = client.wait().await;
                if data.is_none() {
                    continue;
                }
                let res = data.unwrap();
                let opt_result = exch.handle_response(&res, insts_map.as_ref());
                match opt_result {
                    None => info!("Couldn't parse {}", res),
                    Some(MDResponse::Ping) => { client.send(exch.pong().into()).await },
                    Some(MDResponse::Trade(..)) => {}
                    Some(MDResponse::Snapshot(..)) |
                    Some(MDResponse::Delta(..)) => {
                        match sender.try_send(opt_result.unwrap()) {
                            Err(TrySendError::Closed(_)) => { break },
                            Err(TrySendError::Full(el)) => { warn!("Queue overflow, drop {:?}", el) },
                            _ => {},
                        };
                    }
                }
            }
        })
    }

    pub fn spawn_main_loop(sender: Sender<MDResponse>,
                                 mut rx: Receiver<MDResponse>,
                                 mut depthbooks: DepthBookManager) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let msg = rx.recv().await;
                if msg.is_none() { continue; }
                let val = msg.unwrap();
                let inst = val.get_inst().unwrap();
                match depthbooks.update(&inst, val) {
                    Ok(depth) => println!("{:?}", depth),
                    Err(DepthUpdateError::DepthStale) => {
                        match sender.try_send(MDResponse::Snapshot(Self::request_snapshot(&inst).await)) {
                            Ok(_) => {},
                            Err(TrySendError::Closed(_)) => break ,
                            Err(TrySendError::Full(_)) => warn!("Full queue"),
                        };
                    },
                    Err(DepthUpdateError::MissedUpdate) => info!("Missed update for {}", inst.to_raw_string()),
                    Err(DepthUpdateError::UnknownInstrument) => error!("Unexpected instrument update {}", inst.to_raw_string()),
                    Err(DepthUpdateError::StaleUpdate) => {},
                    Err(DepthUpdateError::WaitSnapshot) => {},
                }
            }
        })
    }

    pub fn schedule_snapshots(insts_map: Arc<HashMap<String, Instrument>>,
                                    sender: Sender<MDResponse>) -> JoinHandle<()> {
        tokio::spawn(async move {
            sleep(Duration::from_secs(5)).await;
            for (raw, inst) in insts_map.iter() {
                let snapshot = Runner::request_snapshot(&inst).await;
                let res = sender.send(MDResponse::Snapshot(snapshot)).await;
                if res.is_err() {
                    info!("Termination before snapshot");
                    break;
                }
            }
        })
    }
}
