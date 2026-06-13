# Oxau Sandbox

Oxau Sandbox is a small egui application for editing, checking, and running
Rust-like snippets. It is intentionally built around a shared core so native and
wasm builds use the same diagnostics and execution behavior.

## Run

```sh
cargo run
```

## Build Web

```sh
cargo xtask web build
```

The production bundle is written to `dist/`.

## Develop

```sh
cargo xtask tidy
cargo xtask test
```
