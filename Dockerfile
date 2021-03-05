# Install BPF SDK
FROM solanalabs/rust:latest AS builder
WORKDIR /opt
COPY ./bpf-sdk-install.sh /opt/
RUN ./bpf-sdk-install.sh
RUN /bin/bash -x bin/bpf-sdk/scripts/install.sh

# Build spl-token utility
FROM builder AS token-cli-builder
COPY ./token/cli /opt/token/cli/
COPY ./token/program /opt/token/program/
WORKDIR /opt/token/cli
RUN cargo build --release

# Build spl-memo
# Note: create stub Cargo.toml to speedup build
FROM builder AS spl-memo-builder
COPY ./do.sh Cargo.lock /opt/
COPY ./memo/program/ /opt/memo/program/
WORKDIR /opt/
RUN echo "[workspace]\nmembers = [\n  \"memo/program\",\n]" >Cargo.toml && \
    cat Cargo.toml && \
    /bin/bash -x ./do.sh build memo/program

# Build evm_loader
# Note: create stub Cargo.toml to speedup build
FROM builder AS evm-loader-builder
COPY ./do.sh Cargo.lock /opt/
COPY ./evm_loader/program/ /opt/evm_loader/program/
COPY ./evm_loader/rust-evm/ /opt/evm_loader/rust-evm/
WORKDIR /opt/
RUN echo "[workspace]\nmembers = [\n  \"evm_loader/program\",\n]" >Cargo.toml && \
    cat Cargo.toml && \
    /bin/bash -x ./do.sh build evm_loader/program
RUN ls -l target target/bpfel-unknown-unknown target/bpfel-unknown-unknown/release


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
FROM cybercoredev/solana:latest AS solana
FROM cybercoredev/solana:v1.4.25-resources AS solana-deploy

# Build target image
FROM ubuntu:20.04 AS base
WORKDIR /opt
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get -y install openssl ca-certificates curl python3 python3-pip && \
    rm -rf /var/lib/apt/lists/*

RUN pip3 install solana web3 pysha3
COPY solana-py.patch /tmp/
RUN cd /usr/local/lib/python3.8/dist-packages/ && patch -p0 </tmp/solana-py.patch

COPY --from=solana /opt/solana/bin/solana /opt/solana/bin/solana-keygen /opt/solana/bin/solana-faucet /opt/solana/bin/
COPY --from=solana-deploy /opt/solana/bin/solana /opt/solana/bin/solana-deploy

COPY --from=spl-memo-builder /opt/target/bpfel-unknown-unknown/release/spl_memo.so /opt/
COPY --from=evm-loader-builder /opt/target/bpfel-unknown-unknown/release/evm_loader.so /opt/
COPY --from=token-cli-builder /opt/token/cli/target/release/spl-token /opt/solana/bin/
COPY --from=contracts /opt/ /opt/solidity/
COPY evm_loader/*.py evm_loader/deploy-test.sh /opt/

ENV CONTRACTS_DIR=/opt/solidity/
ENV PATH=/opt/solana/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
