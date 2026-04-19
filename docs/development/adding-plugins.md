# Adding a Plugin

This guide walks through creating a new R3 plugin from scratch.

## 1. Create the Plugin Directory

```
src/plugins/myplugin/
└── mod.rs
```

## 2. Implement the Plugin Trait

```rust
use async_trait::async_trait;
use crate::core::context::Context;
use crate::events::Event;
use crate::plugins::traits::Plugin;
use std::collections::HashMap;

pub struct MyPlugin {
    enabled: bool,
    greeting: String,
}

impl MyPlugin {
    pub fn new(settings: &HashMap<String, String>) -> Self {
        let greeting = settings
            .get("greeting")
            .cloned()
            .unwrap_or_else(|| "Hello!".to_string());

        Self {
            enabled: true,
            greeting,
        }
    }
}

#[async_trait]
impl Plugin for MyPlugin {
    fn name(&self) -> &str {
        "myplugin"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    async fn handle_event(
        &mut self,
        event: &Event,
        ctx: &Context,
    ) -> anyhow::Result<()> {
        match event {
            Event::ClientConnect { cid, name, .. } => {
                let msg = format!("{} {}", self.greeting, name);
                ctx.rcon_say(&msg).await?;
            }
            _ => {}
        }
        Ok(())
    }
}
```

## 3. Register the Module

Add to `src/plugins/mod.rs`:

```rust
pub mod myplugin;
```

## 4. Register in main.rs

Add plugin construction and registration in `src/main.rs`:

```rust
use crate::plugins::myplugin::MyPlugin;

// Inside the plugin registration block:
if let Some(cfg) = find_plugin_config("myplugin", &config.plugins) {
    if cfg.enabled {
        let plugin = MyPlugin::new(&cfg.settings);
        registry.register(Box::new(plugin));
    }
}
```

## 5. Configure

Add to your `referee.toml`:

```toml
[[plugins]]
name = "myplugin"
enabled = true

[plugins.settings]
greeting = "Welcome to the server,"
```

## Plugin Trait Methods

| Method | Purpose |
|--------|---------|
| `name()` | Unique plugin identifier |
| `is_enabled()` | Whether the plugin processes events |
| `enable()` / `disable()` | Toggle plugin state |
| `handle_event()` | Core event handler |

## Available Context Methods

Inside `handle_event`, the `Context` provides:

| Method | Description |
|--------|-------------|
| `ctx.rcon_say(msg)` | Public server message |
| `ctx.rcon_tell(cid, msg)` | Private message to player |
| `ctx.rcon_bigtext(msg)` | Large center-screen text |
| `ctx.rcon(cmd)` | Raw RCON command |
| `ctx.clients()` | Access the client manager |
| `ctx.storage()` | Database access |

## Event Types

Common events your plugin can handle:

| Event | Fired When |
|-------|-----------|
| `ClientConnect` | Player connects |
| `ClientDisconnect` | Player disconnects |
| `ClientUserinfo` | Player info updated |
| `Kill` | A kill occurs |
| `Say` | Public chat message |
| `SayTeam` | Team chat message |
| `ClientSpawn` | Player spawns |
| `GameMapChange` | Map changes |
| `GameRoundStart` | Round begins |

See `src/events/mod.rs` for the complete list of 60+ events.

## Tips

- Keep `handle_event` fast — avoid blocking operations
- Use `tracing` macros for logging: `info!`, `debug!`, `warn!`, `error!`
- Store mutable state in your struct fields
- Use `HashMap<String, String>` settings from config for user-configurable values
- Look at existing plugins like `welcome` or `firstkill` for simple examples
