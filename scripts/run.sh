#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
RUNNER=scalar-runner

consensus_cluster() {
  docker exec -it ${RUNNER} /entry.sh consensus_cluster
}

scalar_cluster() {
  docker exec -it ${RUNNER} /entry.sh scalar_cluster
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