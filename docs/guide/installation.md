# Installation

## Quick Install (Recommended)

The easiest way to install R3 is with the self-extracting installer. It includes the prebuilt binary, example config, and walks you through setup interactively:

```bash
curl -sSL https://r3.pugbot.net/api/updates/install-r3.sh -o install-r3.sh
sudo bash install-r3.sh
```

The installer supports three modes: **Standalone** (single server), **Master** (multi-server hub), or **Client** (managed by a master).

::: tip
The installer is updated automatically with every release. It always contains the latest binary.
:::

## Prerequisites

- **A running Urban Terror 4.3 server** with RCON enabled and `g_logsync 1`

The following are only needed if building from source:

- **Rust** 1.70+ (2021 edition) — [Install Rust](https://rustup.rs/)
- **Node.js** 18+ and npm — required to build the web dashboard frontend

## Building from Source

### Full Release Build (Recommended)

The web dashboard frontend is embedded into the Rust binary at compile time. Build the frontend first, then the backend:

```bash
# Build the frontend
cd ui
npm install
npm run build
cd ..

# Build the backend (includes embedded frontend)
cargo build --release
```

The release binary will be at `target/release/rusty-rules-referee`.

### Backend Only

If you don't need the web dashboard:

```bash
cargo build --release
```

### Development Build

```bash
# Backend with debug symbols
cargo build

# Frontend dev server with hot reload (proxies API to localhost:8080)
cd ui
npm install
npm run dev
```

## Build Scripts

R3 includes convenience build scripts:

**Linux/macOS:**
```bash
./build.sh
```

**Windows (PowerShell):**
```powershell
.\build.ps1
```

Both scripts handle the full build pipeline: install npm dependencies → build frontend → build Rust release binary.

## Running Tests

```bash
cargo test
```

## Cross-Compilation

To build for a Linux server from a different platform, install the target and use cross-compilation:

```bash
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```

::: tip
For cross-compiling from macOS/Windows to Linux, consider using [cross](https://github.com/cross-rs/cross) which handles the toolchain automatically.
:::

## Next Steps

- [Quick Start](/guide/quick-start) — Configure and run R3 for the first time
