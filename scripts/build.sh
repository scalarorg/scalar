#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
PROFILE=debug
RUNNER=scalar-runner
BUILDER=scalar-builder

BIN_DIR=${SCRIPT_DIR}/validator/${PROFILE}
SCALAR_DIR=${SCRIPT_DIR}/../../scalar
TOFND_DIR=${SCRIPT_DIR}/../../../tofnd

reth() {
    BIN_NAME=reth
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

scalar() {
    BIN_NAME=scalar-node
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

consensus() {
    BIN_NAME=consensus-node
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

test_cluster() {
    BIN_NAME=test-cluster
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

reth_test_cluster() {
    BIN_NAME=reth-test-cluster
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

sui() {
    BIN_NAME=sui
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/sui/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

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

$@