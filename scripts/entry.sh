#!/bin/sh

reth() {
    RUST_LOG=info /usr/local/bin/reth node \
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

consensus() {
    RUST_LOG=debug /usr/local/bin/consensus-node \
        --config-path /scalar/validator.yaml
}

test_cluster() {
    RUST_LOG=info /usr/local/bin/test-cluster \
        --cluster-size 4 \
        --consensus-grpc-port 9090 \
        --epoch-duration-ms 3600000
}   

reth_test_cluster() {
    RUST_LOG=info /usr/local/bin/reth-test-cluster
}   

test_cluster1() {
    RUST_LOG=debug /usr/local/bin/test-cluster \
        --config-dir /scalar/cluster-local \
        --epoch-duration-ms 3600000
}   

$@