#!/bin/sh

mkdir -p release
~/.cargo/bin/cross build --release --target x86_64-unknown-linux-gnu
~/.cargo/bin/cross build --release --target x86_64-pc-windows-gnu
mv target/x86_64-unknown-linux-gnu/release/fleet-renderer release
mv target/x86_64-pc-windows-gnu/release/fleet-renderer.exe release
