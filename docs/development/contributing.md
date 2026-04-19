# Contributing

Thanks for your interest in contributing to R3!

## Building

### Backend (Rust)

```sh
cargo build
```

### Frontend (SvelteKit Dashboard)

```sh
cd ui
npm install
npm run build
```

The frontend is embedded into the Rust binary at compile time via `rust_embed`. Run `npm run build` before `cargo build` to include the latest UI.

### Full Release Build

```sh
cd ui && npm run build && cd ..
cargo build --release
```

## Running Tests

```sh
cargo test
```

## Running in Development

```sh
# Backend only
cargo run -- referee.toml

# Frontend dev server (with hot reload)
cd ui && npm run dev
```

## Adding a Plugin

1. Create a new directory under `src/plugins/your_plugin/`
2. Add a `mod.rs` implementing the `Plugin` trait
3. Export it from `src/plugins/mod.rs`
4. Register it in `src/main.rs`

See [Adding Plugins](./adding-plugins) for a full walkthrough.

## Adding an API Endpoint

1. Create or edit a file under `src/web/api/`
2. Declare the module in `src/web/api/mod.rs`
3. Register the route in `src/web/mod.rs` inside `build_router()`
4. Use `AuthUser` or `AdminOnly` extractors for access control

## Adding a Database Migration

1. Create a new `.sql` file under `migrations/` (e.g., `005_your_feature.sql`)
2. Add the migration to `run_migrations()` in both `src/storage/sqlite.rs` and `src/storage/mysql.rs`
3. Add any new types to `src/core/types.rs` and export them from `src/core/mod.rs`
4. Add trait methods to `src/storage/mod.rs` and implement in both backends

## Code Style

- Standard Rust conventions — `cargo fmt` and `cargo clippy`
- `async-trait` for async trait methods
- `anyhow::Result` for application code, `thiserror` for library errors
- `tracing` macros for logging (`info!`, `error!`, `debug!`)
- Frontend: Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`), Tailwind CSS

## Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b my-feature`
3. Make your changes
4. Run `cargo fmt` and `cargo clippy`
5. Run `cargo test`
6. Build the frontend: `cd ui && npm run build`
7. Open a pull request against `main`

## License

By contributing, you agree that your contributions will be licensed under the GPL-2.0-or-later license.
