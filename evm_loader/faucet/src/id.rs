//! Faucet id module.

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::error;

/// Builds a (hopefully) unique string to mark requests.
pub fn generate() -> String {
    let since = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => since,
        Err(err) => {
            error!("generate_id: time went backwards? {}", err);
            Duration::default()
        }
    };
    let digest = md5::compute(since.as_nanos().to_string());
    format!("[{}]", &format!("{:x}", digest)[..7])
}
