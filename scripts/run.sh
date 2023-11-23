#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
RUNNER=scalar-runner

reth() {
  docker exec -it ${RUNNER} /entry.sh reth
}

scalar() {
  docker exec -it ${RUNNER} /entry.sh scalar
}

scalar_cluster_test() {
  docker exec -it ${RUNNER} /entry.sh scalar_cluster_test
}

tss() {
  docker exec -it ${RUNNER} /entry.sh tss
}

relayer() {
  docker exec -it ${RUNNER} /entry.sh relayer
}

$@