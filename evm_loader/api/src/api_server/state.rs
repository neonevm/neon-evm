use crate::Config;

pub struct State {
    pub config: Config,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}
