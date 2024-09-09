use crate::lob::order_book::DepthUpdateError;
use crate::lob::orderbooks::DepthBookManager;
use crate::scheme::connector::{HTTPApi, MarketQueries, WssStream};
use crate::structure::{Exchange, Instrument, MDResponse, Snapshot};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

pub struct Runner;

impl Runner {
    pub async fn request_snapshot(exch: &(dyn HTTPApi + Sync), inst: &Instrument) -> Snapshot {
        let raw = inst.to_raw_string();

        info!("Request depthbook for {}", raw);
        let resp = exch.request_depth_shapshot(inst.clone()).await;
        info!("Got response for {} {:?}", raw, &resp);
        resp
    }

    fn get_streams() -> Vec<WssStream> {
        vec![WssStream::Depth, WssStream::Trade]
    }

    pub fn create_connection(
        exch: Arc<dyn MarketQueries + Send + Sync>,
        sender: Sender<MDResponse>,
        insts: Arc<Vec<Instrument>>,
        insts_map: Arc<HashMap<String, Instrument>>,
    ) -> JoinHandle<()> {
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
                // debug!("Receive: {:?}", res);
                let opt_result = exch.handle_response(&res, insts_map.as_ref());
                match opt_result {
                    None => info!("Couldn't parse {}", res),
                    Some(MDResponse::Ping) => client.send(exch.pong().into()).await,
                    Some(MDResponse::Trade(..)) => {}
                    Some(MDResponse::Snapshot(..)) | Some(MDResponse::Delta(..)) => {
                        match sender.try_send(opt_result.unwrap()) {
                            Err(TrySendError::Closed(_)) => break,
                            Err(TrySendError::Full(el)) => {
                                warn!("Queue overflow, drop {:?}", el)
                            }
                            _ => {}
                        };
                    }
                }
            }
        })
    }

    pub fn spawn_main_loop(
        exch: Vec<(Exchange, Box<dyn HTTPApi + Send + Sync>)>,
        sender: Sender<MDResponse>,
        mut rx: Receiver<MDResponse>,
        mut depthbooks: DepthBookManager,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let msg = rx.recv().await;
                if msg.is_none() {
                    continue;
                }
                let val = msg.unwrap();
                let inst = val
                    .get_inst()
                    .expect("instrument should be available for all types of updates coming here");
                match depthbooks.update(&inst, val) {
                    Ok(depth) => println!("{}", depth),
                    Err(DepthUpdateError::DepthStale) => {
                        let http_api = exch.iter().find(|(e, _)| &inst.exchange == e);
                        match http_api {
                            None => {}
                            Some((_exchange, api)) => {
                                match sender.try_send(MDResponse::Snapshot(
                                    Self::request_snapshot(api.as_ref(), &inst).await,
                                )) {
                                    Ok(_) => {}
                                    Err(TrySendError::Closed(_)) => break,
                                    Err(TrySendError::Full(_)) => warn!("Full queue"),
                                };
                            }
                        }
                    }
                    Err(DepthUpdateError::MissedUpdate) => {
                        info!("Missed update for {}", inst.to_raw_string())
                    }
                    Err(DepthUpdateError::StaleUpdate) => {
                        debug!("Stale update for {}", inst.to_raw_string())
                    }
                    Err(DepthUpdateError::UnknownInstrument) => {
                        error!("Unexpected instrument update {}", inst.to_raw_string())
                    }
                    Err(DepthUpdateError::WaitSnapshot) => {}
                }
            }
        })
    }
}
