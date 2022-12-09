mod base;

use solana_sdk::{message::Message};
use crate::{service::Config, emulator::ToSolanaTransaction};



pub struct Player<'a> {
    config: &'a Config,
    trace_code: Option<String>,
}


