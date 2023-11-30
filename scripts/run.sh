#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
RUNNER=scalar-runner

reth() {
  docker exec -it ${RUNNER} /entry.sh reth
}

scalar() {
  docker exec -it ${RUNNER} /entry.sh scalar
}

test_validator() {
   docker exec -it ${RUNNER} /entry.sh test_validator
}

tss() {
  docker exec -it ${RUNNER} /entry.sh tss
}

relayer() {
  docker exec -it ${RUNNER} /entry.sh relayer
}

$@