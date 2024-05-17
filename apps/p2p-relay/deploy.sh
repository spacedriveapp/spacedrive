#!/bin/bash
# A temporary script to deploy the p2p-relay to the server for testing

set -e

SERVER=""
TARGET_DIR=$(cargo metadata | jq -r .target_directory)
cargo zigbuild --target aarch64-unknown-linux-musl --release

scp "$TARGET_DIR/aarch64-unknown-linux-musl/release/sd-p2p-relay" ec2-user@$SERVER:/home/ec2-user/sd-p2p-relay

# ssh ec2-user@$SERVER
# ./sd-p2p-relay init
#   Enter the `P2P_SECRET` secret env var from Vercel
# ./sd-p2p-relay
