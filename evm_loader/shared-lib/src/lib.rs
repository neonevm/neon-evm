mod param_types;

use std::sync::Arc;

use abi_stable::{
    export_root_module,
    prefix_type::WithMetadata,
    sabi_extern_fn,
    std_types::{RStr, RString},
};
use async_ffi::FutureExt;
use neon_interface::{
    types::{NeonLibError, RNeonResult},
    NeonLib, NeonLib_Ref,
};
use neon_lib::{
    commands::{
        cancel_trx, collect_treasury, create_ether_account, deposit, emulate,
        get_ether_account_data, get_neon_elf, get_storage_at, init_environment,
    },
    config::create_from_api_config,
    context, NeonError,
};
use solana_client::nonblocking::rpc_client::RpcClient;

const _MODULE_WM_: &WithMetadata<NeonLib> = &WithMetadata::new(NeonLib {
    hash,
    get_version,
    invoke,
});

const MODULE: NeonLib_Ref = NeonLib_Ref(_MODULE_WM_.static_as_prefix());

#[export_root_module]
pub fn get_root_module() -> NeonLib_Ref {
    MODULE
}

#[sabi_extern_fn]
fn hash() -> RString {
    env!("NEON_REVISION").into()
}

#[sabi_extern_fn]
fn get_version() -> RString {
    env!("CARGO_PKG_VERSION").into()
}

#[sabi_extern_fn]
fn invoke<'a>(method: RStr<'a>, params: RStr<'a>) -> RNeonResult<'a> {
    async move {
        dispatch(method.as_str(), params.as_str())
            .await
            .map(RString::from)
            .map_err(neon_error_to_rstring)
            .into()
    }
    .into_local_ffi()
}

async fn dispatch(method: &str, params: &str) -> Result<String, NeonError> {
    match method {
        "cancel_trx" => {
            let param_types::Params {
                api_options,
                slot,
                params: param_types::CancelTrx { storage_account },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            let context = context::create(rpc_client.as_ref(), &config);
            let signer = context.signer()?;
            Ok(serde_json::to_string(
                &cancel_trx::execute(
                    context.rpc_client,
                    signer.as_ref(),
                    config.evm_loader,
                    &storage_account,
                )
                .await?,
            )
            .unwrap())
        }
        "collect_treasury" => {
            let param_types::Params {
                api_options,
                slot,
                params: _,
            }: param_types::Params<Option<serde_json::Value>> =
                serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            let context = context::create(rpc_client.as_ref(), &config);
            Ok(
                serde_json::to_string(&collect_treasury::execute(&config, &context).await?)
                    .unwrap(),
            )
        }
        "create_ether_account" => {
            let param_types::Params {
                api_options,
                slot,
                params: param_types::CreateEtherAccount { ether_address },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            let context = context::create(rpc_client.as_ref(), &config);
            let rpc_client = context
                .rpc_client
                .as_any()
                .downcast_ref::<RpcClient>()
                .unwrap();
            let signer = context.signer()?;
            Ok(serde_json::to_string(
                &create_ether_account::execute(
                    rpc_client,
                    config.evm_loader,
                    signer.as_ref(),
                    &ether_address,
                )
                .await?,
            )
            .unwrap())
        }
        "deposit" => {
            let param_types::Params {
                api_options,
                slot,
                params:
                    param_types::Deposit {
                        amount,
                        ether_address,
                    },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            let context = context::create(rpc_client.as_ref(), &config);
            let rpc_client = context
                .rpc_client
                .as_any()
                .downcast_ref::<RpcClient>()
                .unwrap();
            let signer = context.signer()?;
            Ok(serde_json::to_string(
                &deposit::execute(
                    rpc_client,
                    config.evm_loader,
                    signer.as_ref(),
                    amount,
                    &ether_address,
                )
                .await?,
            )
            .unwrap())
        }
        "emulate" => {
            let param_types::Params {
                api_options,
                slot,
                params:
                    param_types::Emulate {
                        evm_loader,
                        tx_params,
                        token_mint,
                        chain_id,
                        step_limit,
                        commitment,
                        accounts,
                        solana_accounts,
                        block_overrides,
                        state_overrides,
                    },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            Ok(serde_json::to_string(
                &emulate::execute(
                    rpc_client.as_ref(),
                    evm_loader,
                    tx_params,
                    token_mint,
                    chain_id,
                    step_limit,
                    commitment,
                    &accounts,
                    &solana_accounts,
                    &block_overrides,
                    state_overrides,
                )
                .await?,
            )
            .unwrap())
        }
        "get_ether_account_data" => {
            let param_types::Params {
                api_options,
                slot,
                params: param_types::GetEtherAccountData { ether_address },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            Ok(serde_json::to_string(
                &get_ether_account_data::execute(
                    rpc_client.as_ref(),
                    &config.evm_loader,
                    &ether_address,
                )
                .await?,
            )
            .unwrap())
        }
        "get_neon_elf" => {
            let param_types::Params {
                api_options,
                slot,
                params: param_types::GetNeonElf { program_location },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            let context = context::create(rpc_client.as_ref(), &config);
            Ok(serde_json::to_string(
                &get_neon_elf::execute(&config, &context, program_location.as_deref()).await?,
            )
            .unwrap())
        }
        "get_storage_at" => {
            let param_types::Params {
                api_options,
                slot,
                params:
                    param_types::GetStorageAt {
                        ether_address,
                        index,
                    },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            Ok(serde_json::to_string(
                &get_storage_at::execute(
                    rpc_client.as_ref(),
                    &config.evm_loader,
                    ether_address,
                    &index,
                )
                .await?,
            )
            .unwrap())
        }
        "init_environment" => {
            let param_types::Params {
                api_options,
                slot,
                params:
                    param_types::InitEnvironment {
                        send_trx,
                        force,
                        keys_dir,
                        file,
                    },
            } = serde_json::from_str(params).map_err(|_| params_to_neon_error(params))?;
            let config = Arc::new(create_from_api_config(&api_options)?);
            let rpc_client = context::build_rpc_client(&config, slot)?;
            let context = context::create(rpc_client.as_ref(), &config);
            Ok(serde_json::to_string(
                &init_environment::execute(
                    &config,
                    &context,
                    send_trx,
                    force,
                    keys_dir.as_deref(),
                    file.as_deref(),
                )
                .await?,
            )
            .unwrap())
        }
        _ => Err(params_to_neon_error(method)),
    }
}

fn params_to_neon_error(params: &str) -> NeonError {
    NeonError::EnvironmentError(
        neon_lib::commands::init_environment::EnvironmentError::InvalidProgramParameter(
            params.into(),
        ),
    )
}

fn neon_error_to_neon_lib_error(error: NeonError) -> NeonLibError {
    assert!(error.error_code() >= 0);
    NeonLibError {
        code: error.error_code() as u32,
        message: error.to_string(),
        data: None,
    }
}

fn neon_error_to_rstring(error: NeonError) -> RString {
    RString::from(serde_json::to_string(&neon_error_to_neon_lib_error(error)).unwrap())
}
