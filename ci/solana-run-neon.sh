#!/usr/bin/env bash

set -em

NEON_BIN=/opt

EVM_LOADER_AUTHORITY_KEYPAIR=${NEON_BIN}/evm_loader-keypair.json
EVM_LOADER_PROGRAM_ID_KEYPAIR=${NEON_BIN}/evm_loader-keypair.json
EVM_LOADER=$(solana address -k ${EVM_LOADER_PROGRAM_ID_KEYPAIR})
EVM_LOADER_PATH=${NEON_BIN}/evm_loader.so

METAPLEX=metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s
METAPLEX_PATH=${NEON_BIN}/metaplex.so

TEST_INVOKE_PROGRAM_ID_KEYPAIR=${NEON_BIN}/neon_test_invoke_program-keypair.json
TEST_INVOKE=$(solana address -k ${TEST_INVOKE_PROGRAM_ID_KEYPAIR})
TEST_INVOKE_PATH=${NEON_BIN}/neon_test_invoke_program.so

VALIDATOR_ARGS=(
  --reset
  --warp-slot 1
  --log-messages-bytes-limit 50000
  --ticks-per-slot 16
  --upgradeable-program ${EVM_LOADER} ${EVM_LOADER_PATH} ${EVM_LOADER_AUTHORITY_KEYPAIR}
  --bpf-program ${METAPLEX} ${METAPLEX_PATH}
  --bpf-program ${TEST_INVOKE} ${TEST_INVOKE_PATH}
)

if [[ -n $GEYSER_PLUGIN_CONFIG ]]; then
  echo "Using geyser plugin with config: $GEYSER_PLUGIN_CONFIG"
  VALIDATOR_ARGS+=(--geyser-plugin-config $GEYSER_PLUGIN_CONFIG)
fi

export RUST_LOG=solana_runtime::system_instruction_processor=trace,solana_runtime::message_processor=debug,solana_bpf_loader=debug,solana_rbpf=debug
solana-test-validator "${VALIDATOR_ARGS[@]}" > /dev/null &
./wait-for-solana.sh ${SOLANA_WAIT_TIMEOUT:-60}

neon-cli --url http://localhost:8899 --evm_loader $EVM_LOADER \
  --commitment confirmed \
  --keypair ${EVM_LOADER_AUTHORITY_KEYPAIR} \
  --loglevel trace init-environment --send-trx --keys-dir /opt/keys

tail +1f test-ledger/validator.log
