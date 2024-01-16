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
  export ROOT=${SCRIPT_DIR}/..
  export RUNTIME=${ROOT}/runtime
  COMMAND=${1:-up -d}
  mkdir -p ${RUNTIME}/genesis/scalar
  declare -a nodes=("node1" "node2" "node3" "node4")
  declare -a validators=("validator1" "validator2" "validator3" "validator4")
  for node in "${nodes[@]}"
  do
   mkdir ${RUNTIME}/${node}/scalar/config/consensus_db
   mkdir ${RUNTIME}/${node}/scalar/config/authorities_db
  done
  docker-compose -f ${SCRIPT_DIR}/../docker/docker-cluster.yml --env-file ${RUNTIME}/.env $COMMAND
}

blockscout() {
  export RUNTIME=${SCRIPT_DIR}/../runtime/blockscout
  COMMAND=${1:-up -d}
  docker-compose -f ${SCRIPT_DIR}/../docker/blockscout/docker-compose.yml --env-file ${RUNTIME}/.env $COMMAND
}

builder() {
  docker exec -it ${BUILDER} bash
}

runner() {
  docker exec -it ${RUNNER} bash
}

$@