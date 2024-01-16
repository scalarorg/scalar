#!/bin/sh

# Create config for 4 genesis validator nodes 
init_genesis() {
    /usr/local/bin/sui genesis --write-config ${SUI_CONFIG_DIR}/genesis.yaml 
}
# Generate validator configs from modified genesis.yaml
init_validators() {
    /usr/local/bin/sui genesis --force \
        --from-config /opt/scalar/genesis.yaml \
        --working-dir ${SUI_CONFIG_DIR} 
}
# Generate random validator configs for testing
generate_test_validators() {
    /usr/local/bin/sui genesis --force \
        --working-dir ${SUI_CONFIG_DIR} 
}
# Start validator with config generated in genesis step
start_validator() {
    /usr/local/bin/scalar-validator \
         --config-path /opt/scalar/config/validator.yaml
}

init_validator() {
    /usr/local/bin/sui genesis --force --epoch-duration-ms 3600000 \
        --working-dir ${SUI_CONFIG_DIR} 
    cd /opt/scalar/key-pairs    
    #/usr/local/bin/sui keytool generate ed25519    
    /usr/local/bin/sui validator make-validator-info ${NAME} "${DESCRIPTION}" ${IMAGE_URL} ${PROJECT_URL} ${HOSTNAME} ${GAS_PRICE}
   
    # /usr/local/bin/scalar-validator \
    #     --config-path /opt/scalar/validator.yaml
}

validator_cluster() {
    RUST_LOG=info /usr/local/bin/validator-cluster \
        --cluster-size 4 \
        --consensus-rpc-host 0.0.0.0 \
        --consensus-rpc-port 9090 \
        --epoch-duration-ms 3600000
}  

move_fullnode_cluster() {
    RUST_LOG=info /usr/local/bin/move-fullnode-cluster \
        --cluster-size 4 \
        --consensus-rpc-port 9090 \
        --epoch-duration-ms 3600000
}   

reth_test_cluster() {
    RUST_LOG=info /usr/local/bin/reth-test-cluster
}  

reth_test_client() {
    TX_COUNT=${1:-20}
    RUST_LOG=info /usr/local/bin/reth-test-client send_raw_tx ${TX_COUNT}
}  

scalar_validator() {
    RUST_LOG=info /usr/local/bin/scalar-validator \
        --consensus-rpc-host 0.0.0.0 \
        --consensus-rpc-port 9090 \
        --epoch-duration-ms 3600000
}

scalar_reth() {
    RUST_LOG=info /usr/local/bin/scalar-reth node \
        --chain dev \
        --http \
        --http.addr 0.0.0.0 \
        --http.corsdomain "*" \
        --http.api admin,debug,eth,net,trace,txpool,web3,rpc \
        --ws \
        --ws.addr 0.0.0.0 \
        --metrics 127.0.0.1:9001 \
        --consensus.enable \
        --consensus.port 9090
}

scalar() {
    RUST_LOG=debug /usr/local/bin/scalar-node \
        --config-path /scalar/validator.yaml \
        #--epoch-duration-ms 3600000 \
        --fullnode-rpc-port 9000 \
        --faucet-port 9123 \
        --indexer-rpc-port 9124
}

reth() {
    RUST_LOG=trace /usr/local/bin/reth node \
        --chain dev \
        --http \
        --http.addr 0.0.0.0 \
        --http.corsdomain "*" \
        --http.api admin,debug,eth,net,trace,txpool,web3,rpc \
        --ws \
        --ws.addr 0.0.0.0 \
        --metrics 127.0.0.1:9001 \
        --auto-mine 
}

consensus() {
    RUST_LOG=debug /usr/local/bin/consensus-node \
        --config-path /scalar/validator.yaml
}


test_cluster() {
    RUST_LOG=debug /usr/local/bin/test-cluster \
        --config-dir /scalar/cluster-local \
        --epoch-duration-ms 3600000
}   

$@