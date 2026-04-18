use async_trait::async_trait;
use crate::core::context::BotContext;
use crate::events::Event;

/// Static metadata about a plugin.
/// Equivalent to Python B3's `PluginData` / class-level attributes.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// The plugin's unique name (used as a key).
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Whether this plugin requires a configuration file.
    pub requires_config: bool,
    /// Names of other plugins this one depends on.
    pub requires_plugins: &'static [&'static str],
    /// Parser names this plugin is compatible with (empty = all).
    pub requires_parsers: &'static [&'static str],
    /// Storage protocols this plugin supports (empty = any / none needed).
    pub requires_storage: &'static [&'static str],
    /// Plugins that should be loaded before this one (soft dependency).
    pub load_after: &'static [&'static str],
}

/// The Plugin trait — every B3 plugin must implement this.
/// Equivalent to Python B3's `Plugin` base class.
///
/// Lifecycle:
///   1. `info()` — return static metadata
///   2. `on_load_config()` — load plugin-specific config
///   3. `on_startup()` — register events, initialize state
///   4. `on_event()` — handle events as they arrive
///   5. `on_disable()` / `on_enable()` — toggle plugin
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Return static metadata about this plugin.
    fn info(&self) -> PluginInfo;

    /// Called when the plugin configuration should be loaded.
    async fn on_load_config(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called once after config is loaded — register event handlers here.
    async fn on_startup(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Handle an incoming event with access to the bot context.
    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()>;

    /// Called when the plugin is enabled.
    fn on_enable(&mut self) {
        // default: no-op
    }

    /// Called when the plugin is disabled.
    fn on_disable(&mut self) {
        // default: no-op
    }

    /// Whether the plugin is currently enabled.
    fn is_enabled(&self) -> bool;

    /// Which events this plugin wants to receive (by key string).
    /// Return `None` to receive all events.
    fn subscribed_events(&self) -> Option<Vec<String>> {
        None // default: receive everything
    }
}
