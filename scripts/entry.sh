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
        --metrics 127.0.0.1:9001
}

scalar() {
    RUST_LOG=info /usr/local/bin/scalar-node \
        --config-path /scalar/fullnode.yaml \
        #--epoch-duration-ms 3600000 \
        --fullnode-rpc-port 9000 \
        --faucet-port 9123 \
        --indexer-rpc-port 9124
}

test_validator() {
    RUST_LOG=info /usr/local/bin/sui-test-validator \
        # --config-path /scalar/fullnode.yaml \
        --epoch-duration-ms 3600000 \
        --fullnode-rpc-port 9000 \
        --faucet-port 9123 \
        --indexer-rpc-port 9124
}

$@