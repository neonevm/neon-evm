ARG SOLANA_IMAGE
# Install BPF SDK
FROM solanalabs/rust:1.69.0 AS builder
RUN cargo install rustfilt
WORKDIR /opt
ARG SOLANA_BPF_VERSION
RUN sh -c "$(curl -sSfL https://release.solana.com/"${SOLANA_BPF_VERSION}"/install)" && \
    /root/.local/share/solana/install/active_release/bin/sdk/sbf/scripts/install.sh
ENV PATH=/root/.local/share/solana/install/active_release/bin:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin


# Build evm_loader
FROM builder AS evm-loader-builder
COPY . /opt/neon-evm/
WORKDIR /opt/neon-evm/evm_loader
ARG REVISION
ENV NEON_REVISION=${REVISION}
RUN cargo fmt --check && \
    cargo clippy --release && \
    cargo build --release && \
    cargo build-bpf --features devnet && cp target/deploy/evm_loader.so target/deploy/evm_loader-devnet.so && \
    cargo build-bpf --features testnet && cp target/deploy/evm_loader.so target/deploy/evm_loader-testnet.so && \
    cargo build-bpf --features govertest && cp target/deploy/evm_loader.so target/deploy/evm_loader-govertest.so && \
    cargo build-bpf --features govertest,emergency && cp target/deploy/evm_loader.so target/deploy/evm_loader-govertest-emergency.so && \
    cargo build-bpf --features mainnet && cp target/deploy/evm_loader.so target/deploy/evm_loader-mainnet.so && \
    cargo build-bpf --features mainnet,emergency && cp target/deploy/evm_loader.so target/deploy/evm_loader-mainnet-emergency.so && \
    cargo build-bpf --features ci --dump

# Build Solidity contracts
FROM ethereum/solc:0.8.0 AS solc
FROM ubuntu:20.04 AS contracts
RUN apt-get update && \
    DEBIAN_FRONTEND=nontineractive apt-get -y install xxd && \
    rm -rf /var/lib/apt/lists/* /var/lib/apt/cache/*
COPY tests/contracts/*.sol /opt/
COPY tests/eof-contracts/*.binary /opt/eof-contracts/
COPY solidity/*.sol /opt/
#COPY evm_loader/tests/test_solidity_precompiles.json /opt/
COPY --from=solc /usr/bin/solc /usr/bin/solc
WORKDIR /opt/
RUN solc --optimize --optimize-runs 200 --output-dir . --bin *.sol && \
    for file in $(ls *.bin); do xxd -r -p $file >${file}ary; done && \
    ls -l

# Define solana-image that contains utility
FROM ${SOLANA_IMAGE} AS solana

# Build target image
FROM ubuntu:20.04 AS base
WORKDIR /opt
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get -y install vim less openssl ca-certificates curl python3 python3-pip parallel && \
    rm -rf /var/lib/apt/lists/*

COPY tests/requirements.txt /tmp/
RUN pip3 install -r /tmp/requirements.txt

#COPY /evm_loader/solidity/ /opt/contracts/contracts/
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

RUN /opt/solana/bin/solana program dump metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s /opt/solana/bin/metaplex.so --url mainnet-beta

COPY evm_loader/solana-run-neon.sh \
     /opt/solana/bin/

COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/deploy/evm_loader*.so /opt/
COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/deploy/evm_loader-dump.txt /opt/
COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/release/neon-cli /opt/
COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/release/neon-api /opt/
COPY --from=solana /usr/bin/spl-token /opt/spl-token
COPY --from=contracts /opt/ /opt/solidity/
COPY --from=contracts /usr/bin/solc /usr/bin/solc
COPY ci/wait-for-solana.sh \
    ci/wait-for-neon.sh \
    ci/deploy-evm.sh \
    ci/deploy-test.sh \
    ci/create-test-accounts.sh \
    ci/evm_loader-keypair.json \
    /opt/

COPY ci/operator-keypairs/ /opt/operator-keypairs
COPY tests /opt/tests
COPY ci/operator-keypairs/id.json /root/.config/solana/id.json
COPY ci/operator-keypairs/id2.json /root/.config/solana/id2.json
COPY ci/keys/ /opt/keys

#ENV CONTRACTS_DIR=/opt/solidity/
ENV PATH=/opt/solana/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/opt
