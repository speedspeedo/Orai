## Description

| Chain       | Contract                                                        |
| ----------- | --------------------------------------------------------------- |
| malaga-420  | wasm13srsm7zrvnyf77atxq8vvtj4tm8nuqfqlfut7me4fml00zxjqd3qmz3dnt |
| osmo-test-4 | osmo1arvynavhgcn7ssrxgtymxnvs2djkawlutw4zmercyhjw8jdd40wq76hl7n |

## Deployment

### Osmoisis chain:

1. Setup

```
cosmwast-std version: 1.0.0
osmosisd version: 11.0.0
```

2. Deploy a smart contract:

Add environment

```sh
OSM_RPC="https://rpc.test.osmosis.zone:443"
OSM_CHAIN_ID="osmo-test-4"
OSM_NODE=(--node $OSM_RPC)
```

```Build
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.11

```sh
cd ver_1.0.0/cw_template/artifacts

RES=$(oraid tx wasm store artifacts/cw_template.wasm --node "https://testnet-rpc.orai.io:443" --chain-id "Oraichain-testnet" --from yodan-wallet --gas-prices 0.1orai --gas auto --gas-adjustment 1.3 -y --output json -b block)

CODE_ID=$(echo $RES | jq -r '.logs[0].events[-1].attributes[-1].value')

echo $CODE_ID

oraid tx wasm instantiate $CODE_ID '{ "lock_periods": [1, 1, 1, 1, 1], "nft_contract": "", "oraiswap_contract": { "orai_contract":  "orai1ks4uwwqfqjyufkwsgllyh7d24jtux7klj3s5gfhsrq586jsajd3q7m93gu", "usdt_contract": "orai1nxk30lxuy9l9qgqshe0tsrq5h6pcjjrhgev09h43keaxhqlwvkfqg4pcj0"}, "validators": [{ "address": "", "weight": 100 }], "deposits": ["100", "50", "10", "1"], "admin": "orai1tmw35y8wuyp8pne7q2mckwq97wymgheudj7dss"}' --node "https://testnet-rpc.orai.io:443" --chain-id Oraichain-testnet --from yodan-wallet --label "cw_counter" --gas-prices 0.025orai --gas auto --gas-adjustment 1.3 -b block -y --no-admin

oraid tx wasm execute b3JhaTE3eHBmdmFrbTJhbWc5NjJ5bHM2Zjg0ejNrZWxsOGM1bHIyNHIydw== '{"mint":{"recipient":"'"orai1z8ghpjllnjnqv04e799pjf83vmfw384ujgecqv45q69hxr9va8jsun5g27"'","amount":"100"}}' --node "https://testnet-rpc.orai.io:443" --chain-id Oraichain-testnet --from yodan-wallet --broadcast-mode=block --gas auto --gas-adjustment 1.3 -y


```

### Malaga chain (WIP):
