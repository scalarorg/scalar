#!/bin/sh

reth() {
    RUST_LOG=DEBUG /usr/local/bin/reth node \
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
    RUST_LOG=debug /usr/local/bin/scalar-node
}

$@