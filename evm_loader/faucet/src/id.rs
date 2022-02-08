//! Faucet id module.

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::error;

/// Builds a (hopefully) unique string to mark requests.
pub fn generate() -> ReqId {
    let since = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => since,
        Err(err) => {
            error!("generate_id: time went backwards? {}", err);
            Duration::default()
        }
    };
    let digest = md5::compute(since.as_nanos().to_string());
    ReqId {
        id: format!("{:x}", digest)[..7].to_string(),
    }
}

/// Represents some context: request id.
#[derive(Clone)]
pub struct ReqId {
    id: String,
}

use std::fmt;

impl fmt::Display for ReqId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{\"req_id\": \"{}\"}}", self.id)
    }
}
