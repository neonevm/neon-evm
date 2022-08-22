#!/bin/bash -eu

# An attempt to mitigate the problem with slow compilation of curve25519-dalek
# https://github.com/rust-lang/rust/issues/95240
export RUSTC_FORCE_INCREMENTAL=0
export CARGO_INCREMENTAL=0

export NEON_REVISION=$(git rev-parse HEAD);

cd evm_loader
cargo fuzz build -O --debug-assertions

FUZZ_TARGET_OUTPUT_DIR=fuzz/target/x86_64-unknown-linux-gnu/release
for f in fuzz/fuzz_targets/*.rs
do
    FUZZ_TARGET_NAME=$(basename ${f%.*})
    cp $FUZZ_TARGET_OUTPUT_DIR/$FUZZ_TARGET_NAME $OUT/
done
