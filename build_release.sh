#!/bin/sh

~/.cargo/bin/cross build --release --target x86_64-unknown-linux-gnu
~/.cargo/bin/cross build --release --target x86_64-pc-windows-gnu
mv target/x86_64-unknown-linux-gnu/release/fleet-renderer .
mv target/x86_64-pc-windows-gnu/release/fleet-renderer.exe .
