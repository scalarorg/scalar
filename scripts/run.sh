#!/bin/sh
SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
RUNNER=scalar-runner

tss() {
  docker exec -it ${RUNNER} /entry.sh tss
}

validator() {
  docker exec -it ${RUNNER} /entry.sh validator
}

relayer() {
  docker exec -it ${RUNNER} /entry.sh relayer
}


$@