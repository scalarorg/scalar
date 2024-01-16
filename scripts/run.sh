#!/bin/bash
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
RUNNER=scalar-runner

validators() {
  export RUNTIME=${SCRIPT_DIR}/../runtime
  export ROOT=${SCRIPT_DIR}/..
  docker exec validator1 /usr/local/bin/sui genesis --force \
      --from-config /opt/scalar/genesis.yaml \
      --working-dir /opt/scalar/config
  declare -a validators=("validator1" "validator2" "validator3" "validator4")
  docker cp validator1:/opt/scalar/config/genesis.blob ${RUNTIME}/genesis.blob
  for validator in "${validators[@]}"
  do
    docker cp ${RUNTIME}/genesis.blob ${validator}:/opt/scalar/config/genesis.blob
    docker cp validator1:/opt/scalar/config/${validator}-8080.yaml ${RUNTIME}/${validator}-8080.yaml
    docker cp ${RUNTIME}/${validator}-8080.yaml ${validator}:/opt/scalar/validator.yaml
    echo "finish create config for $validator"
    #docker exec $validator /entry.sh start_validator
  done
}


validator_cluster() {
  docker exec -it ${RUNNER} /entry.sh validator_cluster
}

move_fullnode_cluster() {
  docker exec -it ${RUNNER} /entry.sh move_fullnode_cluster
}

# HuongND 2023-12-14
reth_test_cluster() {
  docker exec -it scalar-runner rm -rf /root/.local/share/reth
  docker exec -it ${RUNNER} /entry.sh reth_test_cluster
}

reth_test_client() {
  TX_COUNT=${1:-20}
  docker exec -it ${RUNNER} /entry.sh reth_test_client ${TX_COUNT}
}

scalar_reth() {
  docker exec -it ${RUNNER} rm -rf /root/.local/share/reth/dev
  docker exec -it ${RUNNER} /entry.sh scalar_reth
}

reth() {
  docker exec -it ${RUNNER} /entry.sh reth
}

scalar() {
  docker exec -it ${RUNNER} /entry.sh scalar
}

consensus() {
  docker exec -it ${RUNNER} /entry.sh consensus
}

tss() {
  docker exec -it ${RUNNER} /entry.sh tss
}

relayer() {
  docker exec -it ${RUNNER} /entry.sh relayer
}

$@