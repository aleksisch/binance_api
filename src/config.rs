use crate::structure::Exchange;
use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct ExchangeConfig {
    exchange: Exchange,
    pub http_api: String,
    pub exchange_info: String,
    pub snapshot: String,
    pub wss_api: String,
}

#[derive(Deserialize)]
pub struct MDConfig {
    endpoint: Vec<ExchangeConfig>,
}

impl MDConfig {
    pub fn new(config_path: String) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::with_name(config_path.as_ref()))
            .build()?
            .try_deserialize()
    }

    pub fn get(&self, exch: Exchange) -> Option<&ExchangeConfig> {
        self.endpoint.iter().find(|x| x.exchange == exch)
    }
}
