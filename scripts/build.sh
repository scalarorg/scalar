#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
PROFILE=debug
RUNNER=scalar-runner
BUILDER=scalar-builder

BIN_DIR=${SCRIPT_DIR}/validator/${PROFILE}
SCALAR_DIR=${SCRIPT_DIR}/../../scalar
TOFND_DIR=${SCRIPT_DIR}/../../../tofnd

relayer() {
    BUILDER=scalar-relayer
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar-relayer/Cargo.toml --profile dev
    docker cp ${BUILDER}:/scalar-relayer/target/${PROFILE}/scalar-relayer ./scalar-relayer
    docker cp ./scalar-relayer ${RUNNER}:/usr/local/bin/scalar-relayer
    rm ./scalar-relayer
}

tss() {
    docker exec ${BUILDER} rustup component add rustfmt --toolchain 1.70-x86_64-unknown-linux-gnu
    docker exec ${BUILDER} cargo build --manifest-path /tofnd/Cargo.toml --profile dev
    docker cp ${BUILDER}:/tofnd/target/${PROFILE}/tofnd ./tofnd
    docker cp ./tofnd ${RUNNER}:/usr/local/bin/scalar-tofnd
    rm ./tofnd
}

validator() {
   docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin sui-test-validator
   docker cp ${BUILDER}:/sui/target/${PROFILE}/sui-test-validator ./sui-test-validator
   docker cp ./sui-test-validator ${RUNNER}:/usr/local/bin
   rm ./sui-test-validator
}

$@