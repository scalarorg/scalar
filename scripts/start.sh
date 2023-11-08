#!/bin/bash
DIR="$( cd "$( dirname "$0" )" && pwd )"
OS=$(uname)
BUILDER=scalar-builder
RUNNER=scalar-runner

COMPOSE_FILE="-f ${DIR}/../docker/docker-compose.yaml"
if [ "$OS" == "Darwin" ]
then
    ARCH=$(uname -m)
    if [ "$ARCH" == "arm64" ]
    then
        COMPOSE_FILE="-f ${DIR}/../docker/docker-compose-arm64.yaml"
    fi
fi 

#COMPOSE_FILE="${COMPOSE_FILE} -f ${DIR}/../docker/docker-compose-geth.yaml"
#COMPOSE_FILE="${COMPOSE_FILE} -f ${DIR}/../docker/docker-compose-reth.yaml"

init() {
  docker-compose ${COMPOSE_FILE} build
}

containers() {
  COMMAND=${1:-up}
  if [ "$COMMAND" == "up" ]
  then
    docker-compose ${COMPOSE_FILE} up -d
  else
    docker-compose ${COMPOSE_FILE} down
  fi  
}

builder() {
  docker exec -it ${BUILDER} bash
}

runner() {
  docker exec -it ${RUNNER} bash
}

$@