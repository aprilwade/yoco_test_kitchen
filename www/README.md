# Build instructions

```
# Note that the specific version matters. It has to match the version of
wasm-bindgen that bevy uses.
cargo install -f wasm-bindgen-cli --version 0.2.86

cargo build --release --target wasm32-unknown-unknown
# Using -O3 or Os or higher breaks wasm-bindgen at the moment :(
wasm-opt -O2 -o ./yoco_test_kitchen.wasm target/wasm32-unknown-unknown/release/yoco_test_kitchen.wasm
wasm-bindgen --out-dir ./www/pkg --target web ./yoco_test_kitchen.wasm
```

