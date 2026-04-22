use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::core::{Penalty, PenaltyType};
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// What to do when a player with a bad name is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CensorAction {
    Kick,
    Ban,
}

impl CensorAction {
    fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "ban" => Self::Ban,
            _ => Self::Kick,
        }
    }
}

/// The Censor plugin — filters offensive language from chat.
pub struct CensorPlugin {
    enabled: bool,
    bad_words: Vec<Regex>,
    bad_names: Vec<Regex>,
    warn_message: String,
    max_warnings: u32,
    /// What to do when a bad name is detected.
    name_action: CensorAction,
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
            name_action: CensorAction::Kick,
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

    fn matching_bad_name(&self, name: &str) -> Option<String> {
        self.bad_names
            .iter()
            .find(|re| re.is_match(name))
            .map(|re| re.as_str().to_string())
    }

    /// Record a Penalty row for an automatic censor enforcement action.
    async fn record_penalty(
        &self,
        ctx: &BotContext,
        client_db_id: i64,
        penalty_type: PenaltyType,
        reason: String,
    ) {
        if client_db_id <= 0 {
            return;
        }
        let now = Utc::now();
        let penalty = Penalty {
            id: 0,
            penalty_type,
            client_id: client_db_id,
            admin_id: None,
            duration: None,
            reason,
            keyword: "censor".to_string(),
            inactive: false,
            time_add: now,
            time_edit: now,
            time_expire: None,
            server_id: None,
        };
        if let Err(e) = ctx.storage.save_penalty(&penalty).await {
            warn!(error = %e, client = client_db_id, "Failed to record censor penalty");
        }
    }
}

/// Compile a list of string patterns into regexes, skipping blank entries and
/// warn-logging any that fail to compile. Wraps each pattern with `(?i)` for
/// case-insensitive matching.
fn compile_patterns(label: &str, arr: &[toml::Value]) -> Vec<Regex> {
    let mut out = Vec::new();
    for entry in arr.iter().filter_map(|v| v.as_str()) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            warn!("censor: skipping empty {} entry", label);
            continue;
        }
        match Regex::new(&format!("(?i){}", trimmed)) {
            Ok(re) => out.push(re),
            Err(e) => warn!(
                pattern = %trimmed,
                error = %e,
                "censor: skipping invalid {} regex",
                label
            ),
        }
    }
    out
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
            if let Some(v) = s.get("action").and_then(|v| v.as_str()) {
                self.name_action = CensorAction::from_str(v);
            }
            if let Some(arr) = s.get("bad_words").and_then(|v| v.as_array()) {
                self.bad_words = compile_patterns("bad_words", arr);
            }
            if let Some(arr) = s.get("bad_names").and_then(|v| v.as_array()) {
                self.bad_names = compile_patterns("bad_names", arr);
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            bad_words = self.bad_words.len(),
            bad_names = self.bad_names.len(),
            name_action = ?self.name_action,
            "Censor plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };
        let cid_str = client_id.to_string();

        // Check for bad names on name change
        if let Some(key) = ctx.event_registry.get_key(event.event_type) {
            if key == "EVT_CLIENT_NAME_CHANGE" {
                if let EventData::Text(ref name) = event.data {
                    if let Some(pattern) = self.matching_bad_name(name) {
                        warn!(
                            client = client_id,
                            name = %name,
                            pattern = %pattern,
                            action = ?self.name_action,
                            "Bad name detected"
                        );
                        let short_reason = "Offensive player name";
                        let db_id = ctx
                            .clients
                            .get_by_cid(&cid_str)
                            .await
                            .map(|c| c.id)
                            .unwrap_or(0);
                        let (ptype, action_result) = match self.name_action {
                            CensorAction::Kick => (
                                PenaltyType::Kick,
                                ctx.kick(&cid_str, short_reason).await,
                            ),
                            CensorAction::Ban => (
                                PenaltyType::Ban,
                                ctx.ban(&cid_str, short_reason).await,
                            ),
                        };
                        action_result?;
                        self.record_penalty(
                            ctx,
                            db_id,
                            ptype,
                            format!("Offensive player name (matched {})", pattern),
                        )
                        .await;
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
                    let db_id = ctx
                        .clients
                        .get_by_cid(&cid_str)
                        .await
                        .map(|c| c.id)
                        .unwrap_or(0);
                    self.record_penalty(
                        ctx,
                        db_id,
                        PenaltyType::Kick,
                        "Repeated offensive language".to_string(),
                    )
                    .await;
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
