//! The one place that talks to Google's public translate endpoint. Translation and romanization both
//! come through here, so they share one pace (see `throttle.rs`), retry politely when the service says
//! "too many requests" (429), and stop asking for a minute when it keeps saying so.

use crate::throttle::{backoff, Pacer, MAX_ATTEMPTS};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const URL: &str = "https://translate.googleapis.com/translate_a/single";

fn pacer() -> &'static Mutex<Pacer> {
    static PACER: OnceLock<Mutex<Pacer>> = OnceLock::new();
    PACER.get_or_init(|| Mutex::new(Pacer::new(Instant::now())))
}

/// GET the endpoint with `query` and return the JSON answer.
pub async fn get_json(client: &reqwest::Client, query: &[(&str, &str)]) -> Result<Value, String> {
    let mut attempt = 0u32;
    loop {
        let slot = pacer().lock().unwrap().reserve(Instant::now());
        match slot {
            Err(left) => {
                return Err(format!(
                    "Google is busy (too many requests). FloatNote is waiting; try again in {} s.",
                    left.as_secs() + 1
                ))
            }
            Ok(wait) if !wait.is_zero() => tokio::time::sleep(wait).await,
            Ok(_) => {}
        }

        let resp = client
            .get(URL)
            .query(query)
            .send()
            .await
            .map_err(|e| format!("network error: {e}"))?;
        let status = resp.status();
        if status.is_success() {
            return resp.json::<Value>().await.map_err(|e| format!("bad response: {e}"));
        }

        let too_many = status.as_u16() == 429;
        if (too_many || status.is_server_error()) && attempt + 1 < MAX_ATTEMPTS {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            tokio::time::sleep(backoff(attempt, retry_after)).await;
            attempt += 1;
            continue;
        }
        if too_many {
            pacer().lock().unwrap().trip(Instant::now());
            return Err("Google says too many requests (429). Waiting a minute before asking again.".into());
        }
        return Err(format!("the service returned {status}"));
    }
}
