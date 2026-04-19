use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// A scheduled task: when the trigger event fires, execute the command.
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    /// The event key that triggers this task (e.g., "EVT_GAME_ROUND_START").
    pub event_trigger: String,
    /// The action to perform: either an RCON command or a say message.
    pub action: TaskAction,
}

/// The kind of action a scheduled task performs.
#[derive(Debug, Clone)]
pub enum TaskAction {
    /// Send a public message to the server.
    Say(String),
    /// Execute a raw RCON command.
    Rcon(String),
}

/// Runs configured commands or messages on specific game events
/// (round start, round end, map change, etc.).
pub struct SchedulerPlugin {
    enabled: bool,
    /// List of (event_trigger, action) pairs.
    tasks: Vec<ScheduledTask>,
}

impl SchedulerPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            tasks: Vec::new(),
        }
    }

    /// Add a task that sends a message when the given event fires.
    pub fn add_say_task(&mut self, event_trigger: &str, message: &str) {
        self.tasks.push(ScheduledTask {
            event_trigger: event_trigger.to_string(),
            action: TaskAction::Say(message.to_string()),
        });
    }

    /// Add a task that runs an RCON command when the given event fires.
    pub fn add_rcon_task(&mut self, event_trigger: &str, command: &str) {
        self.tasks.push(ScheduledTask {
            event_trigger: event_trigger.to_string(),
            action: TaskAction::Rcon(command.to_string()),
        });
    }
}

impl Default for SchedulerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for SchedulerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "scheduler",
            description: "Runs commands on game events (round start/end, map change)",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(arr) = s.get("tasks").and_then(|v| v.as_array()) {
                self.tasks.clear();
                for item in arr {
                    if let Some(t) = item.as_table() {
                        let event_trigger = t.get("event_trigger").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let action_type = t.get("action_type").and_then(|v| v.as_str()).unwrap_or("say");
                        let action_value = t.get("action_value").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let action = match action_type {
                            "rcon" => TaskAction::Rcon(action_value),
                            _ => TaskAction::Say(action_value),
                        };
                        self.tasks.push(ScheduledTask { event_trigger, action });
                    }
                }
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(tasks = self.tasks.len(), "Scheduler plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(event_key) = ctx.event_registry.get_key(event.event_type) else {
            return Ok(());
        };

        for task in &self.tasks {
            if task.event_trigger == event_key {
                match &task.action {
                    TaskAction::Say(message) => {
                        info!(event = event_key, message = %message, "Scheduler: saying message");
                        ctx.say(message).await?;
                    }
                    TaskAction::Rcon(command) => {
                        info!(event = event_key, command = %command, "Scheduler: executing RCON command");
                        ctx.write(command).await?;
                    }
                }
            }
        }

        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn on_enable(&mut self) {
        self.enabled = true;
    }

    fn on_disable(&mut self) {
        self.enabled = false;
    }

    fn subscribed_events(&self) -> Option<Vec<String>> {
        if self.tasks.is_empty() {
            return Some(Vec::new());
        }
        let mut events: Vec<String> = self
            .tasks
            .iter()
            .map(|t| t.event_trigger.clone())
            .collect();
        events.sort();
        events.dedup();
        Some(events)
    }
}
