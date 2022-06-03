ARG SOLANA_REVISION
# Install BPF SDK
FROM solanalabs/rust:latest AS builder
RUN rustup toolchain install nightly
RUN rustup component add clippy --toolchain nightly
WORKDIR /opt
RUN sh -c "$(curl -sSfL https://release.solana.com/stable/install)" && \
    /root/.local/share/solana/install/active_release/bin/sdk/bpf/scripts/install.sh
ENV PATH=/root/.local/share/solana/install/active_release/bin:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin


# Build evm_loader
# Note: create stub Cargo.toml to speedup build
FROM builder AS evm-loader-builder
COPY ./evm_loader/ /opt/evm_loader/
WORKDIR /opt/evm_loader
RUN cd program && /opt/evm_loader/ci_checks.sh
ARG REVISION
ENV NEON_REVISION=${REVISION}
RUN cargo +nightly clippy && \
    cargo build --release && \
    cargo build-bpf --features no-logs,devnet && cp target/deploy/evm_loader.so target/deploy/evm_loader-devnet.so && \
    cargo build-bpf --features no-logs,testnet && cp target/deploy/evm_loader.so target/deploy/evm_loader-testnet.so && \
    cargo build-bpf --features no-logs,alpha && cp target/deploy/evm_loader.so target/deploy/evm_loader-alpha.so && \
    cargo build-bpf --features no-logs,mainnet && cp target/deploy/evm_loader.so target/deploy/evm_loader-mainnet.so && \
    cargo build-bpf --features no-logs

# Build Solidity contracts
FROM ethereum/solc:0.7.0 AS solc
FROM ubuntu:20.04 AS contracts
RUN apt-get update && \
    DEBIAN_FRONTEND=nontineractive apt-get -y install xxd && \
    rm -rf /var/lib/apt/lists/* /var/lib/apt/cache/*
COPY evm_loader/tests/*.sol /opt/
COPY evm_loader/tests/test_solidity_precompiles.json /opt/
COPY --from=solc /usr/bin/solc /usr/bin/solc
WORKDIR /opt/
RUN solc --output-dir . --bin *.sol && \
    for file in $(ls *.bin); do xxd -r -p $file >${file}ary; done && \
        ls -l

# Define solana-image that contains utility
FROM solanalabs/solana:${SOLANA_REVISION} AS solana

# Build target image
FROM ubuntu:20.04 AS base
WORKDIR /opt
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get -y install vim less openssl ca-certificates curl python3 python3-pip parallel && \
    rm -rf /var/lib/apt/lists/*

COPY evm_loader/tests/requirements.txt solana-py.patch /tmp/
RUN pip3 install -r /tmp/requirements.txt
RUN cd /usr/local/lib/python3.8/dist-packages/ && patch -p0 </tmp/solana-py.patch

RUN curl -fsSL https://deb.nodesource.com/setup_16.x | bash -
RUN apt update & apt install -y nodejs
RUN npm install -g npm@8.6.0
COPY /evm_loader/solidity/ /opt/contracts/
WORKDIR /opt/contracts
RUN npm install
WORKDIR /opt

COPY --from=solana \
     /usr/bin/solana \
     /usr/bin/solana-validator \
     /usr/bin/solana-keygen \
     /usr/bin/solana-faucet \
     /usr/bin/solana-genesis \
     /usr/bin/solana-run.sh \
     /usr/bin/fetch-spl.sh \
     /usr/bin/spl* \
     /opt/solana/bin/

COPY evm_loader/solana-run-neon.sh \
     /opt/solana/bin/

COPY --from=evm-loader-builder /opt/evm_loader/target/deploy/evm_loader*.so /opt/
COPY --from=evm-loader-builder /opt/evm_loader/target/release/neon-cli /opt/
COPY --from=solana /usr/bin/spl-token /opt/spl-token
COPY --from=contracts /opt/ /opt/solidity/
COPY --from=contracts /usr/bin/solc /usr/bin/solc
COPY evm_loader/*.py \
    evm_loader/tests/*.py \
    evm_loader/wait-for-solana.sh \
    evm_loader/wait-for-neon.sh \
    evm_loader/create-test-accounts.sh \
    evm_loader/deploy-evm.sh \
    evm_loader/deploy-test.sh \
    evm_loader/neon_token_keypair.json \
    evm_loader/permission_allowance_token_keypair.json \
    evm_loader/permission_denial_token_keypair.json \
    evm_loader/utils/set_single_acct_permission.sh \
    evm_loader/utils/set_many_accts_permission.sh \
    evm_loader/deploy-contracts.sh \
    evm_loader/get_deployer_address.py /opt/

COPY evm_loader/evm_loader-keypair.json /opt/
COPY evm_loader/collateral_pool_generator.py evm_loader/collateral-pool-keypair.json /opt/
COPY evm_loader/operator1-keypair.json /root/.config/solana/id.json
COPY evm_loader/operator2-keypair.json /root/.config/solana/id2.json


ENV CONTRACTS_DIR=/opt/solidity/
ENV PATH=/opt/solana/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/opt
