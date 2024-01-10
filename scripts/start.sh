#!/bin/bash
export SCRIPT_DIR="$( cd "$( dirname "$0" )" && pwd )"
OS=$(uname)
BUILDER=scalar-builder
RUNNER=scalar-runner

COMPOSE_FILE="-f ${SCRIPT_DIR}/../docker/docker-compose.yaml"
if [ "$OS" == "Darwin" ]
then
    ARCH=$(uname -m)
    if [ "$ARCH" == "arm64" ]
    then
        COMPOSE_FILE="-f ${SCRIPT_DIR}/../docker/docker-compose-arm64.yaml"
    fi
fi 

#COMPOSE_FILE="${COMPOSE_FILE} -f ${SCRIPT_DIR}/../docker/docker-compose-geth.yaml"
#COMPOSE_FILE="${COMPOSE_FILE} -f ${SCRIPT_DIR}/../docker/docker-compose-reth.yaml"

init() {
  export UID=$(id -u)
  export GID=$(id -g)
  docker-compose ${COMPOSE_FILE} build
}

containers() {
  COMMAND=${1:-up}
  export GID=$(id -g)
  export UID=$(id -u)
  echo $GID, $UID
  if [ "$COMMAND" == "up" ]
  then
    docker-compose ${COMPOSE_FILE} up -d
  else
    docker-compose ${COMPOSE_FILE} down
  fi  
}

cluster() {
  export ROOT=${SCRIPT_DIR}/../runtime
  COMMAND=${1:-up -d}
  docker-compose -f ${SCRIPT_DIR}/../docker/docker-cluster.yaml $COMMAND
}

builder() {
  docker exec -it ${BUILDER} bash
}

runner() {
  docker exec -it ${RUNNER} bash
}

$@