#!/bin/bash
DIR="$( cd "$( dirname "$0" )" && pwd )"
OS=$(uname)
BUILDER=scalar-builder
RUNNER=scalar-runner
COMPOSE_FILE=${DIR}/../docker/docker-compose.yaml
if [ "$OS" == "Darwin" ]
then
    ARCH=$(uname -m)
    if [ "$ARCH" == "arm64" ]
    then
        COMPOSE_FILE=${DIR}/../docker/docker-compose-arm64.yaml
    fi
fi 

init() {
  docker-compose -f ${COMPOSE_FILE} build
}

containers() {
  COMMAND=${1:-up}
  if [ "$COMMAND" == "up" ]
  then
    docker-compose -f ${COMPOSE_FILE} up -d
  else
    docker-compose -f ${COMPOSE_FILE} down
  fi  
}

builder() {
  docker exec -it ${BUILDER} bash
}

runner() {
  docker exec -it ${RUNNER} bash
}

$@