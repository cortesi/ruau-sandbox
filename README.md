# Ruau Sandbox

Ruau Sandbox is a small demonstration app for Ruau. It provides a focused
egui-based workspace for editing Luau snippets, inspecting Ruau diagnostics, and
running code through the same core used by the native and wasm builds.

The app is intentionally modest: it shows how Ruau can be embedded in a
desktop/web interface without adding unrelated product surface.

## Run

```sh
cargo run
cargo run -- path/to/script.luau
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
