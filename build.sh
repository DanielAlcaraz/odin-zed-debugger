#!/bin/bash

# 1. Build the project for the correct WASM target
cargo build --release --target wasm32-wasip1

# 2. Move the output to the root so Zed can see it
cp target/wasm32-wasip1/release/odin_debugger.wasm extension.wasm

echo "Build complete: extension.wasm is ready."
