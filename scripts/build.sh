#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
PROFILE=debug
RUNNER=scalar-runner
BUILDER=scalar-builder

BIN_DIR=${SCRIPT_DIR}/validator/${PROFILE}
SCALAR_DIR=${SCRIPT_DIR}/../../scalar
TOFND_DIR=${SCRIPT_DIR}/../../../tofnd

# Working from 2023-12-13
validator_cluster() {
    BIN_NAME=validator-cluster
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}
move_fullnode_cluster() {
    BIN_NAME=move-fullnode-cluster
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

# HuongND 2023-12-14
reth_test_cluster() {
    BIN_NAME=reth-test-cluster
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/reth-test-cluster/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    docker cp ${BUILDER}:${WORKING_DIR}/test-genesis.json ${SCRIPT_DIR}/test-genesis.json
    docker cp ${SCRIPT_DIR}/test-genesis.json ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
    rm ${SCRIPT_DIR}/test-genesis.json
}

reth_test_client() {
    BIN_NAME=reth-test-client
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/reth-test-cluster/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    docker cp ${BUILDER}:${WORKING_DIR}/test-genesis.json ${SCRIPT_DIR}/test-genesis.json
    docker cp ${SCRIPT_DIR}/test-genesis.json ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
    rm ${SCRIPT_DIR}/test-genesis.json
}

# Working from 2023-12-13
scalar_reth() {
    BIN_NAME=scalar-reth
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

reth() {
    BIN_NAME=reth
    WORKING_DIR=/scalar/reth
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

scalar() {
    BIN_NAME=scalar-node
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

consensus() {
    BIN_NAME=consensus-node
    WORKING_DIR=/scalar
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
    docker cp ${SCRIPT_DIR}/${BIN_NAME} ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/${BIN_NAME}
}

sui() {
    BIN_NAME=sui
    RKING_DIR=/scalar/sui
    docker exec -it ${BUILDER} cargo build --manifest-path ${WORKING_DIR}/Cargo.toml --profile dev --bin ${BIN_NAME}
    docker cp ${BUILDER}:${WORKING_DIR}/target/${PROFILE}/${BIN_NAME} ${SCRIPT_DIR}/${BIN_NAME}
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