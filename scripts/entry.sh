#!/bin/sh

reth() {
    RUST_LOG=info /usr/local/bin/reth node \
        --chain dev \
        --http \
        --http.corsdomain "*" \
        --http.api admin,debug,eth,net,trace,txpool,web3,rpc \
        --ws \
        --metrics 127.0.0.1:9001
}

scalar() {
    RUST_LOG=debug /usr/local/bin/scalar-node
}

$@