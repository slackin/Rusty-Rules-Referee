use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// The Censor plugin — filters offensive language from chat.
pub struct CensorPlugin {
    enabled: bool,
    bad_words: Vec<Regex>,
    bad_names: Vec<Regex>,
    warn_message: String,
    max_warnings: u32,
    /// Per-client warning counts.
    warnings: RwLock<HashMap<i64, u32>>,
}

impl CensorPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            bad_words: Vec::new(),
            bad_names: Vec::new(),
            warn_message: "Watch your language!".to_string(),
            max_warnings: 3,
            warnings: RwLock::new(HashMap::new()),
        }
    }

    /// Add a word pattern to the bad words list.
    pub fn add_bad_word(&mut self, pattern: &str) -> anyhow::Result<()> {
        let re = Regex::new(&format!("(?i){}", regex::escape(pattern)))?;
        self.bad_words.push(re);
        Ok(())
    }

    /// Add a name pattern to the bad names list.
    pub fn add_bad_name(&mut self, pattern: &str) -> anyhow::Result<()> {
        let re = Regex::new(&format!("(?i){}", regex::escape(pattern)))?;
        self.bad_names.push(re);
        Ok(())
    }

    fn contains_bad_word(&self, text: &str) -> bool {
        self.bad_words.iter().any(|re| re.is_match(text))
    }

    fn contains_bad_name(&self, name: &str) -> bool {
        self.bad_names.iter().any(|re| re.is_match(name))
    }
}

impl Default for CensorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CensorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "censor",
            description: "Filters offensive language and player names",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("warn_message").and_then(|v| v.as_str()) {
                self.warn_message = v.to_string();
            }
            if let Some(v) = s.get("max_warnings").and_then(|v| v.as_integer()) {
                self.max_warnings = v as u32;
            }
            if let Some(arr) = s.get("bad_words").and_then(|v| v.as_array()) {
                self.bad_words = arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| regex::Regex::new(&format!("(?i){}", s)).ok())
                    .collect();
            }
            if let Some(arr) = s.get("bad_names").and_then(|v| v.as_array()) {
                self.bad_names = arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| regex::Regex::new(&format!("(?i){}", s)).ok())
                    .collect();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            bad_words = self.bad_words.len(),
            bad_names = self.bad_names.len(),
            "Censor plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };
        let cid_str = client_id.to_string();

        // Check for bad names on name change or connect
        if let Some(key) = ctx.event_registry.get_key(event.event_type) {
            if key == "EVT_CLIENT_NAME_CHANGE" {
                if let EventData::Text(ref name) = event.data {
                    if self.contains_bad_name(name) {
                        warn!(client = client_id, name = %name, "Bad name detected");
                        ctx.kick(&cid_str, "Offensive player name").await?;
                        return Ok(());
                    }
                }
            }
        }

        // Check for bad words in chat
        if let EventData::Text(ref text) = event.data {
            if self.contains_bad_word(text) {
                let count = {
                    let mut warnings = self.warnings.write().await;
                    let count = warnings.entry(client_id).or_insert(0);
                    *count += 1;
                    *count
                };

                warn!(client = client_id, text = %text, warns = count, "Bad word detected");

                if count >= self.max_warnings {
                    ctx.message(&cid_str, "^1Kicked for repeated offensive language").await?;
                    ctx.kick(&cid_str, "Offensive language").await?;
                    self.warnings.write().await.remove(&client_id);
                } else {
                    ctx.message(
                        &cid_str,
                        &format!("^1{} ^7({}/{})", self.warn_message, count, self.max_warnings),
                    ).await?;
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
        Some(vec![
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_CLIENT_NAME_CHANGE".to_string(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_word_detection() {
        let mut censor = CensorPlugin::new();
        censor.add_bad_word("badword").unwrap();
        censor.add_bad_word("offensive").unwrap();

        assert!(censor.contains_bad_word("you are a badword"));
        assert!(censor.contains_bad_word("BADWORD"));
        assert!(censor.contains_bad_word("that's offensive"));
        assert!(!censor.contains_bad_word("hello friend"));
    }

    #[test]
    fn test_bad_name_detection() {
        let mut censor = CensorPlugin::new();
        censor.add_bad_name("slur").unwrap();

        assert!(censor.contains_bad_name("player_slur_123"));
        assert!(censor.contains_bad_name("SLUR"));
        assert!(!censor.contains_bad_name("normalname"));
    }

    #[test]
    fn test_warning_count_tracking() {
        let censor = CensorPlugin::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            // Simulate incrementing warnings for a client
            let client_id: i64 = 42;
            {
                let mut warnings = censor.warnings.write().await;
                let count = warnings.entry(client_id).or_insert(0);
                *count += 1;
                assert_eq!(*count, 1);
            }
            {
                let mut warnings = censor.warnings.write().await;
                let count = warnings.entry(client_id).or_insert(0);
                *count += 1;
                assert_eq!(*count, 2);
            }
            {
                let mut warnings = censor.warnings.write().await;
                let count = warnings.entry(client_id).or_insert(0);
                *count += 1;
                // At max_warnings (3), would trigger kick
                assert_eq!(*count, 3);
                assert!(*count >= censor.max_warnings);
            }
            // After kick, count is removed
            {
                let mut warnings = censor.warnings.write().await;
                warnings.remove(&client_id);
                assert!(!warnings.contains_key(&client_id));
            }
        });
    }

    #[test]
    fn test_multiple_bad_word_patterns() {
        let mut censor = CensorPlugin::new();
        censor.add_bad_word("word1").unwrap();
        censor.add_bad_word("word2").unwrap();
        censor.add_bad_word("word3").unwrap();

        assert!(censor.contains_bad_word("contains word1 here"));
        assert!(censor.contains_bad_word("contains word2 here"));
        assert!(censor.contains_bad_word("contains word3 here"));
        assert!(!censor.contains_bad_word("contains word4 here"));
    }
}
