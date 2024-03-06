#!/bin/bash
# A temporary script to deploy the p2p-relay to the server for testing

SERVER="ec2-13-57-219-49.us-west-1.compute.amazonaws.com"
TARGET_DIR=$(cargo metadata | jq -r .target_directory)
cargo zigbuild --target aarch64-unknown-linux-musl --release

echo "$TARGET_DIR/aarch64-unknown-linux-musl/release/sd-p2p-relay"

scp "$TARGET_DIR/aarch64-unknown-linux-musl/release/sd-p2p-relay" ec2-user@$SERVER:~
