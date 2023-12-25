#!/bin/sh

validator_cluster() {
    RUST_LOG=info /usr/local/bin/validator-cluster \
        --cluster-size 4 \
        --consensus-grpc-port 9090 \
        --epoch-duration-ms 3600000
}  

move_fullnode_cluster() {
    RUST_LOG=info /usr/local/bin/move-fullnode-cluster \
        --cluster-size 4 \
        --consensus-grpc-port 9090 \
        --epoch-duration-ms 3600000
}   

reth_test_cluster() {
    RUST_LOG=info /usr/local/bin/reth-test-cluster
}  

reth_test_client() {
    TX_COUNT=${1:-20}
    RUST_LOG=info /usr/local/bin/reth-test-client send_raw_tx ${TX_COUNT}
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
        --narwhal \
        --narwhal.port 9090
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