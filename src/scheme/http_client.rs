use serde_json::from_str;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use tungstenite::client;

pub struct HTTPClient;

impl HTTPClient {
    pub async fn get<T>(url: &str) -> serde_json::Result<T>
    where
        for<'a> T: serde::Deserialize<'a>,
    {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        let res = client.get(url).await.unwrap();
        let body = res.text().await.unwrap().to_string();
        from_str::<T>(body.as_str())
    }
}
