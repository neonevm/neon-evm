use log::{info, debug};

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
    system_program,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use evm::{H160};

use crate::{
    Config,
    NeonCliResult,
};

/// Executes subcommand `deposit`.
pub fn execute(
    config: &Config,
    amount: u64,
    ether_address: &H160,
) -> NeonCliResult {
    let (ether_pubkey, nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);

    let mut instructions = Vec::with_capacity(2);

    let ether_account = config.rpc_client.get_account(&ether_pubkey);
    if ether_account.is_err() {
        info!("No ether account for {}; will be created", ether_address);
        instructions.push(create_ether_account_instruction(
            config,
            ether_address,
            ether_pubkey,
            nonce
        ));
    }

    let token_mint_id = evm_loader::config::token_mint::id();

    let signer_token_pubkey =
        spl_associated_token_account::get_associated_token_address(&config.signer.pubkey(), &token_mint_id);
    let evm_token_authority = Pubkey::find_program_address(&[b"Deposit"], &config.evm_loader).0;

    instructions.push(spl_approve_instruction(
        config,
        signer_token_pubkey,
        ether_pubkey,
        amount,
    ));

    let evm_pool_pubkey =
        spl_associated_token_account::get_associated_token_address(&evm_token_authority, &token_mint_id);

    instructions.push(deposit_instruction(
        config,
        signer_token_pubkey,
        evm_pool_pubkey,
        ether_address,
        ether_pubkey,
    ));

    let mut finalize_message = Message::new(&instructions, Some(&config.signer.pubkey()));
    let blockhash = config.rpc_client.get_latest_blockhash()?;
    finalize_message.recent_blockhash = blockhash;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &finalize_message
    )?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    info!("{}", serde_json::json!({
        "amount": amount,
        "ether": hex::encode(ether_address),
        "nonce": nonce,
    }));

    Ok(())
}

/// Returns instruction for creation of account.
fn create_ether_account_instruction(
    config: &Config,
    ether_address: &H160,
    solana_address: Pubkey,
    nonce: u8,
) -> Instruction {
    Instruction::new_with_bincode(
        config.evm_loader,
        &(0x1e_u8, ether_address.as_fixed_bytes(), nonce, 0_u32),
        vec![
            AccountMeta::new(config.signer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(solana_address, false),
        ]
    )
}

/// Returns instruction to approve transfer of NEON tokens.
fn spl_approve_instruction(
    config: &Config,
    source_pubkey: Pubkey,
    delegate_pubkey: Pubkey,
    amount: u64,
) -> Instruction {
    use spl_token::instruction::TokenInstruction;

    let accounts = vec![
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new_readonly(delegate_pubkey, false),
        AccountMeta::new_readonly(config.signer.pubkey(), true),
    ];

    let data = TokenInstruction::Approve { amount }.pack();

    Instruction {
        program_id: spl_token::id(),
        accounts,
        data,
    }
}

/// Returns instruction to deposit NEON tokens.
fn deposit_instruction(
    config: &Config,
    source_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    ether_address: &H160,
    ether_account_pubkey: Pubkey,
) -> Instruction {
    Instruction::new_with_bincode(
        config.evm_loader,
        &(0x27_u8, ether_address.as_fixed_bytes()),
        vec![
            AccountMeta::new(source_pubkey, false),
            AccountMeta::new(destination_pubkey, false),
            AccountMeta::new(ether_account_pubkey, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new(config.signer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}
