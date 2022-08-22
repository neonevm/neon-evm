FROM gcr.io/oss-fuzz-base/base-builder-rust:v1
COPY . $SRC/evm_loader
WORKDIR $SRC/evm_loader
COPY .clusterfuzzlite/build.sh $SRC/