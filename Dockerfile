ARG SOLANA_IMAGE
# Install BPF SDK
FROM solanalabs/rust:1.73.0 AS builder
RUN cargo install rustfilt
WORKDIR /opt
ARG SOLANA_BPF_VERSION
RUN sh -c "$(curl -sSfL https://release.solana.com/"${SOLANA_BPF_VERSION}"/install)" && \
    /root/.local/share/solana/install/active_release/bin/sdk/sbf/scripts/install.sh
ENV PATH=${PATH}:/root/.local/share/solana/install/active_release/bin


# Build evm_loader
FROM builder AS evm-loader-builder
COPY .git /opt/neon-evm/.git
COPY evm_loader /opt/neon-evm/evm_loader
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


# Add neon_test_invoke_program to the genesis
FROM neonlabsorg/neon_test_programs:latest AS neon_test_programs

# Define solana-image that contains utility
FROM builder AS base

RUN solana program dump metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s /opt/metaplex.so --url mainnet-beta

COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/deploy/evm_loader*.so /opt/
COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/deploy/evm_loader-dump.txt /opt/
COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/release/neon-cli /opt/
COPY --from=evm-loader-builder /opt/neon-evm/evm_loader/target/release/neon-api /opt/

COPY --from=neon_test_programs /opt/deploy/ /opt/deploy/

COPY ci/wait-for-solana.sh \
    ci/wait-for-neon.sh \
    ci/solana-run-neon.sh \
    ci/deploy-evm.sh \
    ci/create-test-accounts.sh \
    ci/evm_loader-keypair.json \
    /opt/

COPY solidity/ /opt/solidity
COPY ci/operator-keypairs/ /opt/operator-keypairs
COPY ci/operator-keypairs/id.json /root/.config/solana/id.json
COPY ci/operator-keypairs/id2.json /root/.config/solana/id2.json
COPY ci/keys/ /opt/keys

ENV PATH=${PATH}:/opt

ENTRYPOINT [ "/opt/solana-run-neon.sh" ]
