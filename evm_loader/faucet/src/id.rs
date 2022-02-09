//! Faucet id module.

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::error;

/// Returns empty id.
pub fn default() -> ReqId {
    ReqId::default()
}

/// Builds a (hopefully) unique string to mark requests.
pub fn generate() -> ReqId {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |e| {
            error!("{{}} generate_id: time went backwards? {}", e);
            Duration::default()
        },
        |s| s,
    );
    let digest = md5::compute(since.as_nanos().to_string());
    ReqId {
        id: format!("{:x}", digest)[..7].to_string(),
    }
}

/// Represents some context: request id.
#[derive(Default, Clone)]
pub struct ReqId {
    id: String,
}

use std::fmt;

impl fmt::Display for ReqId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.id.is_empty() {
            write!(f, "{{}}")
        } else {
            write!(f, "{{\"req_id\": \"{}\"}}", self.id)
        }
    }
}
