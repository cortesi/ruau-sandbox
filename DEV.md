# Developer Guide

## Native App

```sh
cargo run
cargo run -- path/to/script.rs
```

To expose the eguidev runtime:

```sh
cargo run -p ruau-sandbox-gui --features devtools -- --dev-mcp
cargo run -p ruau-sandbox-gui --features devtools -- --dev-mcp path/to/script.rs
```

## Web

One-time setup:

```sh
cargo xtask web setup
```

Development server:

```sh
cargo xtask web serve
```

Production bundle:

```sh
cargo xtask web build
```

Serve the production bundle:

```sh
cargo xtask web serve-dist 8000
```

## Quality

```sh
cargo xtask tidy
cargo xtask test
```
