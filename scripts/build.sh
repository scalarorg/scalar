#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
PROFILE=debug
RUNNER=scalar-runner
BUILDER=scalar-builder

BIN_DIR=${SCRIPT_DIR}/validator/${PROFILE}
SCALAR_DIR=${SCRIPT_DIR}/../../scalar
TOFND_DIR=${SCRIPT_DIR}/../../../tofnd

reth() {
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin reth
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/reth ${SCRIPT_DIR}/reth
    docker cp ${SCRIPT_DIR}/reth ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/reth
}

scalar() {
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin scalar-node
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/scalar-node ${SCRIPT_DIR}/scalar-node
    docker cp ${SCRIPT_DIR}/scalar-node ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/scalar-node
}

scalar_cluster_test() {
    docker exec -it ${BUILDER} cargo build --manifest-path /scalar/Cargo.toml --profile dev --bin scalar-cluster-test
    docker cp ${BUILDER}:/scalar/target/${PROFILE}/scalar-cluster-test ${SCRIPT_DIR}/scalar-cluster-test
    docker cp ${SCRIPT_DIR}/scalar-cluster-test ${RUNNER}:/usr/local/bin
    rm ${SCRIPT_DIR}/scalar-cluster-test
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