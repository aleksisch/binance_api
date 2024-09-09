use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde_json::from_str;
use std::fmt::format;
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

        let res = client
            .get(url)
            .send()
            .await
            .expect(format!("HTTP request failed {url}").as_str());
        let body = res
            .text()
            .await
            .expect("Conversion of response to text failed")
            .to_string();
        from_str::<T>(body.as_str())
    }
}
