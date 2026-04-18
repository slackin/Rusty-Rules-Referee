# Contributing to Rusty Rules Referee

Thanks for your interest in contributing!

## Building

```sh
cargo build
```

## Running Tests

```sh
cargo test
```

## Adding a Plugin

1. Create a new directory under `src/plugins/your_plugin/`
2. Add a `mod.rs` implementing the `Plugin` trait
3. Export it from `src/plugins/mod.rs`
4. Register it in `src/main.rs`

See the [README](README.md#adding-a-plugin) for a full example.

## Code Style

- Follow standard Rust conventions (`cargo fmt`, `cargo clippy`)
- Use `async-trait` for async trait methods
- Errors: `anyhow::Result` for application code, `thiserror` for library errors
- Logging: use `tracing` macros (`info!`, `error!`, `debug!`)

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b my-feature`)
3. Make your changes
4. Run `cargo fmt` and `cargo clippy`
5. Run `cargo test`
6. Open a pull request against `main`

## License

By contributing, you agree that your contributions will be licensed under the GPL-2.0-or-later license.
