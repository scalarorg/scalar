#!/bin/sh
#HOST=http://192.168.1.239:8545
#HOST=http://127.0.0.1:8545
HOST=http://172.19.0.2:8545
ADDR_SENDER=0xb60e8dd61c5d32be8058bb8eb970870f07233155
ADDR_RECEIVER=0xd46e8dd67c5d32be8058bb8eb970870f07244567

sendTransaction() {
    curl --location ${HOST} \
    --header 'Content-Type: application/json' \
    --data '{
        "jsonrpc":"2.0",
        "method":"eth_sendTransaction",
        "params":[{
            "from": "${ADDR_SENDER}",
            "to": "${ADDR_RECEIVER}",
            "gas": "0x76c0",
            "gasPrice": "0x9184e72a000",
            "value": "0x9184e72a",
            "data": "0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675"
        }],
        "id":1
    }'
}

$@