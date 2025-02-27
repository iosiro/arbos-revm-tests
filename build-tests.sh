#!/bin/bash

rm -rf tests/assets
mkdir -p tests/assets

solc solidity-contracts/Multicaller.sol --bin --output-dir tests/assets

# for each in directory
$(
    cd stylus-contracts
    for dir in *; do
        $(
            cd $dir
            cargo build --release --lib
            cp target/wasm32-unknown-unknown/release/$(echo $dir | tr '-' '_').wasm ../../tests/assets
        )
    done
)

