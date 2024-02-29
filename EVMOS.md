1. Init genesis
   Run scripts/evmos/init-docker.sh in each docker container to generate config for each node
2. Update config
   Collect node_id, and add to seeds config in the config/config.toml
   seeds = "41358a7e68e5add634d453c0c06fa2f141a8fa25@172.22.0.12:26656,d738e7eb9b00389d8343d7dd9b49fa5e27fd2220@172.22.0.13:26656,81182437e73e10a078f8f74d0f7c71c04c36e52c@172.22.0.14:26656"
   Each node's seeds list does not contain its own's id
3. Start node
   Start node with scripts/evmos/start-docker.sh
4. Todo
   Write script for generate node_id
