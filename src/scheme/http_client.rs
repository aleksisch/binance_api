use serde_json::from_str;

pub struct HTTPClient;

impl HTTPClient {
    pub async fn get<T>(url: &str) -> serde_json::Result<T> where for <'a> T: serde::Deserialize<'a> {
        let res = reqwest::get(url)
            .await
            .unwrap();
        let body = res.text().await.unwrap().to_string();
        from_str::<T>(body.as_str())
    }
}