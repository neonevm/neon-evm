# Install BPF SDK
FROM solanalabs/rust:1.53.0 AS builder
RUN rustup component add clippy
WORKDIR /opt
RUN sh -c "$(curl -sSfL https://release.solana.com/v1.7.9/install)" && \
    /root/.local/share/solana/install/releases/1.7.9/solana-release/bin/sdk/bpf/scripts/install.sh
ENV PATH=/root/.local/share/solana/install/active_release/bin:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# Build evm_loader
# Note: create stub Cargo.toml to speedup build
FROM builder AS evm-loader-builder
COPY ./evm_loader/ /opt/evm_loader/
WORKDIR /opt/evm_loader/program
RUN /opt/evm_loader/ci_checks.sh
RUN cargo clippy
RUN cargo build-bpf --features no-logs
WORKDIR /opt/evm_loader/cli
RUN cargo clippy
RUN cargo build --release
WORKDIR /opt/evm_loader/performance/sender
RUN cargo clippy
RUN cargo build --release

# Download and build spl-token
FROM builder AS spl-token-builder
ADD http://github.com/solana-labs/solana-program-library/archive/refs/tags/token-cli-v2.0.14.tar.gz /opt/
RUN tar -xvf /opt/token-cli-v2.0.14.tar.gz && \
    cd /opt/solana-program-library-token-cli-v2.0.14/token/cli && \
    cargo build --release && \
    cp /opt/solana-program-library-token-cli-v2.0.14/target/release/spl-token /opt/

# Build Solidity contracts
FROM ethereum/solc:0.7.0 AS solc
FROM ubuntu:20.04 AS contracts
RUN apt-get update && \
    DEBIAN_FRONTEND=nontineractive apt-get -y install xxd && \
    rm -rf /var/lib/apt/lists/* /var/lib/apt/cache/*
COPY evm_loader/*.sol /opt/
COPY evm_loader/precompiles_testdata.json /opt/
COPY evm_loader/ERC20/src/*.sol /opt/
COPY --from=solc /usr/bin/solc /usr/bin/solc
WORKDIR /opt/
RUN solc --output-dir . --bin *.sol && \
    for file in $(ls *.bin); do xxd -r -p $file >${file}ary; done && \
        ls -l

# Define solana-image that contains utility
FROM cybercoredev/solana:v1.7.9-resources AS solana

# Build target image
FROM ubuntu:20.04 AS base
WORKDIR /opt
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get -y install vim less openssl ca-certificates curl python3 python3-pip parallel && \
    rm -rf /var/lib/apt/lists/*

COPY evm_loader/test_requirements.txt solana-py.patch /tmp/
RUN pip3 install -r /tmp/test_requirements.txt
RUN cd /usr/local/lib/python3.8/dist-packages/ && patch -p0 </tmp/solana-py.patch

COPY --from=solana /opt/solana/bin/solana /opt/solana/bin/solana-keygen /opt/solana/bin/solana-faucet /opt/solana/bin/
COPY --from=evm-loader-builder /opt/evm_loader/program/target/deploy/evm_loader.so /opt/
COPY --from=evm-loader-builder /opt/evm_loader/cli/target/release/neon-cli /opt/
COPY --from=evm-loader-builder /opt/evm_loader/performance/sender/target/release/sender /opt/
COPY --from=spl-token-builder /opt/spl-token /opt/
COPY --from=contracts /opt/ /opt/solidity/
COPY --from=contracts /usr/bin/solc /usr/bin/solc
COPY evm_loader/*.py evm_loader/deploy-test.sh evm_loader/test_token_keypair evm_loader/test_token_owner evm_loader/test_token_config.yml /opt/
COPY evm_loader/ERC20/test/test_*.py /opt/
COPY evm_loader/performance/run.py evm_loader/performance/run.sh evm_loader/performance/deploy-evmloader.sh  /opt/
COPY evm_loader/performance/contracts  /opt/
COPY evm_loader/evm_loader-keypair.json /opt/
COPY evm_loader/collateral_pool_generator.py evm_loader/collateral-pool-keypair.json /opt/


ENV CONTRACTS_DIR=/opt/solidity/
ENV PATH=/opt/solana/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/opt
