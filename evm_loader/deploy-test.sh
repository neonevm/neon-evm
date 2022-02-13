#!/bin/bash

no_wait=false
no_acc=false
no_deploy=false
test_filename=false

# DESC: Usage help
# ARGS: None
# OUTS: None
function script_usage() {
    cat << EOF
Usage:
    -h|--help		Displays this help
    --nowait           Skip wait for solana
    --noacc            Skip create test accounts
    --nodeploy         Skip EVM deploy
   <test_filename>      Custom file with test case

Example:
   deploy-test.sh --nodeploy test.py
EOF
}

# DESC: Exit script with the given message
# ARGS: $1 (required): Message to print on exit
#       $2 (optional): Exit code (defaults to 0)
# OUTS: None
# NOTE: The convention used in this script for exit codes is:
#       0: Normal exit
#       1: Abnormal exit due to external error
#       2: Abnormal exit due to script error
function script_exit() {
    printf '%s\n' "$1"
    if [[ $# -eq 1 ]]; then
        exit 0
    else
        exit "$2"
    fi
}

# DESC: Parameter parser
# ARGS: $@ (optional): Arguments provided to the script
# OUTS: Variables indicating command-line parameters and options 
function parse_params() {
    test_filename=""

    local param
    while [[ $# -gt 0 ]]; do
        param="$1"
        shift
        case $param in
            -h | --help)
                script_usage
                exit 0
                ;;
            --nowait)
                no_wait=true
                ;;
            --noacc)
                no_acc=true
                ;;
            --nodeploy)
                no_deploy=true
                ;;
            *)
                test_filename=$param
                ;;
        esac
    done

    printf "Wait for solana:         %s\n" $([ $no_wait == false ] && echo "Yes" || echo "No")
    printf "Test accounts creation:  %s\n" $([ $no_acc == false ] && echo "Yes" || echo "No")
    printf "EVM ddeploy:             %s\n" $([ $no_deploy == false ] && echo "Yes" || echo "No")
    printf "Custom test filename:    %s\n\n" $([ $test_filename ] && echo $test_filename || echo "No")
}

parse_params "$@"

set -xeuo pipefail

if ! ($no_wait == true || wait-for-solana.sh 20); then
    script_exit "Failed to wait for solana" 1
fi

if ! ($no_acc == true || create-test-accounts.sh 2); then
    script_exit "Failed to create test accounts" 1
fi

if ! ($no_deploy == true || deploy-evm.sh); then
    script_exit "Failed to deploy EVM" 1
fi    

ACCOUNT=$(solana address --keypair /root/.config/solana/id.json)
ACCOUNT2=$(solana address --keypair /root/.config/solana/id2.json)
export ETH_TOKEN_MINT=$(solana address -k neon_token_keypair.json)
export EVM_LOADER=$(solana address -k evm_loader-keypair.json)
export $(neon-cli --evm_loader "$EVM_LOADER" neon-elf-params evm_loader.so)

if ($no_acc == false); then
    TOKEN_ACCOUNT=$(spl-token create-account $ETH_TOKEN_MINT --owner $ACCOUNT | grep -Po 'Creating account \K[^\n]*')
    spl-token mint $ETH_TOKEN_MINT 5000 --owner evm_loader-keypair.json -- $TOKEN_ACCOUNT
    spl-token balance $ETH_TOKEN_MINT --owner $ACCOUNT

    TOKEN_ACCOUNT2=$(spl-token create-account $ETH_TOKEN_MINT --owner $ACCOUNT2 | grep -Po 'Creating account \K[^\n]*')
    spl-token mint $ETH_TOKEN_MINT 5000 --owner evm_loader-keypair.json -- $TOKEN_ACCOUNT2
    spl-token balance $ETH_TOKEN_MINT --owner $ACCOUNT2
fi

echo "Deploy test..."
if ($test_filename); then
    python3 -m unittest discover -v -p $test_filename
else
    python3 -m unittest discover -v -p 'test*.py'
fi

script_exit "Deploy test success"

