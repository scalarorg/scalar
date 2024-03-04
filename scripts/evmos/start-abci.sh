#!/bin/bash

KEYNAME=${KEYNAME}
CHAINID=${CHAINID:-escalar_2024-1}
MONIKER=${MONIKER}
KEYRING=${KEYRING:-test}
EVMOSD=evmosd
DATA_DIR=/opt/abci

rm -rf ${DATA_DIR}/config
rm -rf ${DATA_DIR}/data
rm -rf ${DATA_DIR}/keyring-test

echo "create and add new keys"
${EVMOSD} keys add $KEYNAME --home $DATA_DIR --no-backup --chain-id $CHAINID --algo "eth_secp256k1" --keyring-backend ${KEYRING}
echo "init Evmos with moniker=$MONIKER and chain-id=$CHAINID"
${EVMOSD} init $MONIKER --chain-id $CHAINID --home $DATA_DIR
echo "prepare genesis: Allocate genesis accounts"
${EVMOSD} add-genesis-account \
"$(${EVMOSD} keys show $KEYNAME -a --home $DATA_DIR --keyring-backend ${KEYRING})" 1000000000000000000aevmos,1000000000000000000stake \
--home $DATA_DIR --keyring-backend ${KEYRING}
echo "prepare genesis: Sign genesis transaction"
${EVMOSD} gentx $KEYNAME 1000000000000000000stake --keyring-backend ${KEYRING} --home $DATA_DIR  --chain-id $CHAINID
echo "prepare genesis: Collect genesis tx"
${EVMOSD} collect-gentxs --home $DATA_DIR
echo "prepare genesis: Run validate-genesis to ensure everything worked and that the genesis file is setup correctly"
${EVMOSD} validate-genesis --home $DATA_DIR

echo "starting evmos node $KEYNAME in background ..."
${EVMOSD} start --pruning=nothing --rpc.unsafe --keyring-backend ${KEYRING} --with-tendermint=false \
    --transport="grpc" \
    --home $DATA_DIR #>$DATA_DIR/node.log 2>&1 & disown

echo "started evmos node"
tail -f /dev/null