use std::collections::HashMap;
use tracing::{info, warn};

use super::Plugin;
use crate::core::context::BotContext;

/// Manages all loaded plugins and their lifecycle.
/// Equivalent to the plugin management in the original Python bot's `Parser` class.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
    name_index: HashMap<String, usize>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            name_index: HashMap::new(),
        }
    }

    /// Register a plugin. Checks dependencies before accepting.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> anyhow::Result<()> {
        let info = plugin.info();
        let name = info.name.to_string();

        // Check required plugins are already registered
        for dep in info.requires_plugins {
            if !self.name_index.contains_key(*dep) {
                anyhow::bail!(
                    "Plugin '{}' requires '{}' which is not loaded",
                    name,
                    dep
                );
            }
        }

        info!(plugin = %name, "Registered plugin");
        let idx = self.plugins.len();
        self.plugins.push(plugin);
        self.name_index.insert(name, idx);
        Ok(())
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        self.name_index
            .get(name)
            .map(|&idx| self.plugins[idx].as_ref())
    }

    /// Get a mutable reference to a plugin by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut dyn Plugin> {
        let idx = *self.name_index.get(name)?;
        Some(self.plugins[idx].as_mut())
    }

    /// Initialize all plugins: load config, then startup.
    pub async fn startup_all(&mut self) -> anyhow::Result<()> {
        for plugin in &mut self.plugins {
            let name = plugin.info().name;
            if let Err(e) = plugin.on_load_config().await {
                warn!(plugin = name, error = %e, "Failed to load config");
            }
            plugin.on_startup().await?;
            info!(plugin = name, "Started");
        }
        Ok(())
    }

    /// Dispatch an event to all enabled plugins that subscribe to it.
    pub async fn dispatch(&self, event: &crate::events::Event, ctx: &BotContext) {
        for plugin in &self.plugins {
            if !plugin.is_enabled() {
                continue;
            }

            // Check if the plugin subscribes to this event type
            let should_handle = match plugin.subscribed_events() {
                None => true, // receives everything
                Some(ref keys) => {
                    // Resolve event type ID back to key string and check subscription
                    if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
                        keys.iter().any(|k| k == event_key)
                    } else {
                        false
                    }
                }
            };

            if should_handle {
                if let Err(e) = plugin.on_event(event, ctx).await {
                    warn!(
                        plugin = plugin.info().name,
                        error = %e,
                        "Error handling event"
                    );
                }
            }
        }
    }

    /// Return an iterator over all plugin names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.name_index.keys().map(|s| s.as_str())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
