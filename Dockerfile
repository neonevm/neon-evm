# Install BPF SDK
FROM solanalabs/rust:1.52.0 AS builder
WORKDIR /opt
RUN sh -c "$(curl -sSfL https://release.solana.com/v1.6.9/install)" && \
    /root/.local/share/solana/install/releases/1.6.9/solana-release/bin/sdk/bpf/scripts/install.sh
ENV PATH=/root/.local/share/solana/install/active_release/bin:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# Build spl-token utility
FROM builder AS token-cli-builder
COPY ./token/ /opt/token/
COPY ./associated-token-account /opt/associated-token-account
WORKDIR /opt/token/cli
RUN cargo build --release

# Build spl-memo
# Note: create stub Cargo.toml to speedup build
FROM builder AS spl-memo-builder
COPY ./memo/program/ /opt/memo/program/
WORKDIR /opt/memo/program
RUN cd /opt/memo/program && cargo build-bpf

# Build evm_loader
# Note: create stub Cargo.toml to speedup build
FROM builder AS evm-loader-builder
COPY ./evm_loader/ /opt/evm_loader/
WORKDIR /opt/evm_loader/program
RUN cargo build-bpf --features no-logs
RUN cd ../cli && cargo build --release

# Build Solidity contracts
FROM ethereum/solc:0.5.12 AS solc
FROM ubuntu:20.04 AS contracts
RUN apt-get update && \
    DEBIAN_FRONTEND=nontineractive apt-get -y install xxd && \
    rm -rf /var/lib/apt/lists/* /var/lib/apt/cache/*
COPY evm_loader/*.sol /opt/
COPY --from=solc /usr/bin/solc /usr/bin/solc
WORKDIR /opt/
RUN solc --output-dir . --bin *.sol && \
    for file in $(ls *.bin); do xxd -r -p $file >${file}ary; done && \
        ls -l

# Define solana-image that contains utility
FROM cybercoredev/solana:9e68ded325b8ceb25d346cd6e2656f790ba89033 AS solana

# Build target image
FROM ubuntu:20.04 AS base
WORKDIR /opt
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get -y install openssl ca-certificates curl python3 python3-pip && \
    rm -rf /var/lib/apt/lists/*

COPY evm_loader/test_requirements.txt solana-py.patch /tmp/
RUN pip3 install -r /tmp/test_requirements.txt
RUN cd /usr/local/lib/python3.8/dist-packages/ && patch -p0 </tmp/solana-py.patch

COPY --from=solana /opt/solana/bin/solana /opt/solana/bin/solana-keygen /opt/solana/bin/solana-faucet /opt/solana/bin/
COPY --from=spl-memo-builder /opt/memo/program/target/deploy/spl_memo.so /opt/
COPY --from=evm-loader-builder /opt/evm_loader/program/target/deploy/evm_loader.so /opt/
COPY --from=evm-loader-builder /opt/evm_loader/cli/target/release/neon-cli /opt
COPY --from=token-cli-builder /opt/token/cli/target/release/spl-token /opt/solana/bin/
COPY --from=contracts /opt/ /opt/solidity/
COPY evm_loader/*.py evm_loader/deploy-test.sh /opt/

ENV CONTRACTS_DIR=/opt/solidity/
ENV PATH=/opt/solana/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/opt
