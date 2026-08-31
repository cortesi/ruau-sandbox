# Ruau Sandbox

Ruau Sandbox is a small demonstration app for Ruau. It provides a focused
egui-based workspace for editing Luau snippets, inspecting Ruau diagnostics, and
running code through the same core used by the native and wasm builds.

The app is intentionally modest: it shows how Ruau can be embedded in a
desktop/web interface without adding unrelated product surface.

Click the screenshot to try the [live sandbox][live-sandbox] in your browser.

[![Ruau Sandbox running the default area example][sandbox-screenshot]][live-sandbox]

[live-sandbox]: https://cortesi.github.io/ruau-sandbox/
[sandbox-screenshot]: docs/images/readme-screenshot-img_0.jpg

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
ncode tidy
ncode test
```

Regenerate the README screenshot:

```sh
edev eval --out-dir docs/images scripts/readme-screenshot.luau
```
