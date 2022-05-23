#!/usr/bin/env bash

set -e

SOLANA_BIN=/opt/solana/bin
NEON_BIN=/opt

function deploy_tokens() {
    # deploy tokens needed by Neon EVM
    export SKIP_EVM_DEPLOY="YES"
    export SOLANA_URL=http://127.0.0.1:8899

    cd ${NEON_BIN}
    ./wait-for-solana.sh 20
    ./deploy-evm.sh
}

deploy_tokens &

# run Solana with Neon EVM in genesis

cd ${SOLANA_BIN}

EVM_LOADER_SO=evm_loader.so
EVM_LOADER=$(${SOLANA_BIN}/solana address -k ${NEON_BIN}/evm_loader-keypair.json)
EVM_LOADER_PATH=${NEON_BIN}/${EVM_LOADER_SO}

cp ${EVM_LOADER_PATH} .

NEON_BPF_ARGS=(
    --bpf-program ${EVM_LOADER} BPFLoader2111111111111111111111111111111111 ${EVM_LOADER_SO}
)

export SOLANA_RUN_SH_GENESIS_ARGS="${NEON_BPF_ARGS[@]}"

./solana-run.sh
