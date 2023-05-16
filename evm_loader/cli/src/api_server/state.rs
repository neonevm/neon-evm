use crate::Config;
use std::sync::Arc;

#[derive(Clone)]
pub struct State {
    pub config: Arc<Config>,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}
