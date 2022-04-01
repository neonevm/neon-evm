//! Tools for tracing runtime events

use evm::Context;
use evm::{H160, H256, U256};
use evm_runtime::{CreateScheme, ExitReason, Transfer};

environmental::environmental!(listener: dyn EventListener + 'static);

/// Implementors can build traces based on handled [Events](Event)
pub trait EventListener {
    /// Handle an [Event]
    fn event(&mut self, event: Event);
}

/// Trace event
#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
    /// Call event
    Call {
        /// Called code address
        code_address: H160,
        /// Transfer parameters
        transfer: &'a Option<Transfer>,
        /// Input data provided to the call
        input: &'a [u8],
        /// Target gas
        target_gas: Option<u64>,
        /// Static call flag
        is_static: bool,
        /// Runtime context
        context: &'a Context,
    },
    /// Create event
    Create {
        /// Creator address
        caller: H160,
        /// Address of the created account
        address: H160,
        /// Scheme
        scheme: CreateScheme,
        /// Value the created account is endowed with
        value: U256,
        /// Init code
        init_code: &'a [u8],
        /// Target Gas
        target_gas: Option<u64>,
    },
    /// Suicide event
    Suicide {
        /// Suicided address
        address: H160,
        /// Suicided contract heir
        target: H160,
        /// Balance before suicide
        balance: U256,
    },
    /// Exit event
    Exit {
        /// Exit reason
        reason: &'a ExitReason,
        /// Return value
        return_value: &'a [u8],
    },
    /// Transactional Call event
    TransactCall {
        /// Caller account address
        caller: H160,
        /// Destination account address
        address: H160,
        /// Value transferred to the destination account
        value: U256,
        /// Input data provided to the call
        data: &'a [u8],
        /// Gas Limit
        gas_limit: U256,
    },
    /// Transactional Create event
    TransactCreate {
        /// Creator address
        caller: H160,
        /// Value the created account is endowed with
        value: U256,
        /// Init code
        init_code: &'a [u8],
        /// Gas limit
        gas_limit: U256,
        /// Address of the created account
        address: H160,
    },
    /// Transactional Create2 event
    TransactCreate2 {
        /// Creator address
        caller: H160,
        /// Value the created account is endowed with
        value: U256,
        /// Init code
        init_code: &'a [u8],
        /// Salt
        salt: H256,
        /// Gas limit
        gas_limit: U256,
        /// Address of the created account
        address: H160,
    },
}

// Expose `listener::with` to the crate only
pub(crate) fn with<F: FnOnce(&mut (dyn EventListener + 'static))>(f: F) {
    listener::with(f);
}

/// Run closure with provided listener
pub fn using<R, F: FnOnce() -> R>(new: &mut (dyn EventListener + 'static), f: F) -> R {
    listener::using(new, f)
}
