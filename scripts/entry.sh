#!/bin/sh

reth() {
    RUST_LOG=info /usr/local/bin/reth node \
        --chain dev
}

scalar() {
    RUST_LOG=debug /usr/local/bin/scalar-node
}

$@