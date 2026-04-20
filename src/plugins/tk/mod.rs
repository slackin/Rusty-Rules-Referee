use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

#[derive(Debug, Default)]
struct TkRecord {
    team_kills: u32,
    team_damage: f32,
}

/// A single team kill event, tracking attacker and victim for the forgive system.
#[derive(Debug, Clone)]
struct TkEvent {
    attacker_id: i64,
    attacker_name: String,
    forgiven: bool,
}

/// The TK (Team Kill) plugin — monitors and penalizes team killing.
/// Includes a forgive system: victims can forgive their killers to prevent penalties.
pub struct TkPlugin {
    enabled: bool,
    max_team_kills: u32,
    max_team_damage: f32,
    /// Per-client TK tracking (attacker_id -> record). Interior mutability via RwLock.
    records: RwLock<HashMap<i64, TkRecord>>,
    /// Per-victim TK history: victim_id -> list of TK events against them (most recent last).
    tk_victims: RwLock<HashMap<i64, Vec<TkEvent>>>,
}

impl TkPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_team_kills: 5,
            max_team_damage: 300.0,
            records: RwLock::new(HashMap::new()),
            tk_victims: RwLock::new(HashMap::new()),
        }
    }

    /// Get the number of unforgiven TKs for an attacker.
    async fn unforgiven_tk_count(&self, attacker_id: i64) -> u32 {
        let victims = self.tk_victims.read().await;
        let mut count = 0u32;
        for events in victims.values() {
            for ev in events {
                if ev.attacker_id == attacker_id && !ev.forgiven {
                    count += 1;
                }
            }
        }
        count
    }
}

impl Default for TkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for TkPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "tk",
            description: "Monitors and penalizes team killing / team damage",
            requires_config: true,
            requires_plugins: &["admin"],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("max_team_kills").and_then(|v| v.as_integer()) {
                self.max_team_kills = v as u32;
            }
            if let Some(v) = s.get("max_team_damage").and_then(|v| v.as_float()) {
                self.max_team_damage = v as f32;
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            max_tk = self.max_team_kills,
            max_td = self.max_team_damage,
            "TK plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // Check if this is a round start event — reset records
        if let Some(key) = ctx.event_registry.get_key(event.event_type) {
            if key == "EVT_GAME_ROUND_START" {
                let mut records = self.records.write().await;
                records.clear();
                let mut victims = self.tk_victims.write().await;
                victims.clear();
                info!("TK records cleared for new round");
                return Ok(());
            }

            // Handle forgive commands from chat
            if key == "EVT_CLIENT_SAY" || key == "EVT_CLIENT_TEAM_SAY" {
                if let EventData::Text(ref text) = event.data {
                    if let Some(cmd) = text.strip_prefix('!') {
                        let parts: Vec<&str> = cmd.splitn(2, char::is_whitespace).collect();
                        let command = parts[0].to_lowercase();
                        let _args = parts.get(1).map(|s| s.trim()).unwrap_or("");
                        let Some(issuer_id) = event.client_id else {
                            return Ok(());
                        };
                        let cid_str = issuer_id.to_string();

                        match command.as_str() {
                            "forgive" | "f" => {
                                // Forgive the last person who TK'd you
                                let mut victims = self.tk_victims.write().await;
                                if let Some(events) = victims.get_mut(&issuer_id) {
                                    if let Some(last_unforgiven) = events.iter_mut().rev().find(|e| !e.forgiven) {
                                        last_unforgiven.forgiven = true;
                                        let name = last_unforgiven.attacker_name.clone();
                                        ctx.say(&format!("^7{} ^2has forgiven ^7{}", 
                                            ctx.clients.get_by_cid(&cid_str).await
                                                .map(|c| c.name.clone()).unwrap_or_else(|| "Player".to_string()),
                                            name
                                        )).await?;
                                    } else {
                                        ctx.message(&cid_str, "^7No one to forgive").await?;
                                    }
                                } else {
                                    ctx.message(&cid_str, "^7No one to forgive").await?;
                                }
                            }

                            "forgivelist" | "fl" => {
                                // List all unforgiven TKs against you
                                let victims = self.tk_victims.read().await;
                                if let Some(events) = victims.get(&issuer_id) {
                                    let unforgiven: Vec<&TkEvent> = events.iter().filter(|e| !e.forgiven).collect();
                                    if unforgiven.is_empty() {
                                        ctx.message(&cid_str, "^7No unforgiven team kills against you").await?;
                                    } else {
                                        ctx.message(&cid_str, "^3Unforgiven TKs against you:").await?;
                                        for (i, ev) in unforgiven.iter().enumerate() {
                                            ctx.message(&cid_str, &format!("  ^7{}. ^1{}", i + 1, ev.attacker_name)).await?;
                                        }
                                    }
                                } else {
                                    ctx.message(&cid_str, "^7No team kills recorded against you").await?;
                                }
                            }

                            "forgiveall" | "fa" => {
                                // Forgive all TKs against you
                                let mut victims = self.tk_victims.write().await;
                                if let Some(events) = victims.get_mut(&issuer_id) {
                                    let mut count = 0u32;
                                    for ev in events.iter_mut() {
                                        if !ev.forgiven {
                                            ev.forgiven = true;
                                            count += 1;
                                        }
                                    }
                                    if count > 0 {
                                        let name = ctx.clients.get_by_cid(&cid_str).await
                                            .map(|c| c.name.clone()).unwrap_or_else(|| "Player".to_string());
                                        ctx.say(&format!("^7{} ^2has forgiven all team kills ^7({} forgiven)", name, count)).await?;
                                    } else {
                                        ctx.message(&cid_str, "^7Nothing to forgive").await?;
                                    }
                                } else {
                                    ctx.message(&cid_str, "^7Nothing to forgive").await?;
                                }
                            }

                            "forgiveinfo" | "fi" => {
                                // Show info about your unforgiven TK penalties
                                let count = self.unforgiven_tk_count(issuer_id).await;
                                let remaining = if self.max_team_kills > count { self.max_team_kills - count } else { 0 };
                                ctx.message(&cid_str, &format!(
                                    "^7You have ^1{} ^7unforgiven TKs. ^3{} ^7more before kick.",
                                    count, remaining
                                )).await?;
                            }

                            "forgiveclear" | "fc" => {
                                // Admin: clear all TK records for a target player
                                // Requires mod level
                                if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
                                    if client.max_level() >= 20 {
                                        if _args.is_empty() {
                                            ctx.message(&cid_str, "Usage: !forgiveclear <player>").await?;
                                        } else {
                                            let matches = ctx.clients.find_by_name(_args).await;
                                            match matches.len() {
                                                0 => {
                                                    ctx.message(&cid_str, &format!("^7No player found matching ^3{}", _args)).await?;
                                                }
                                                1 => {
                                                    let target = &matches[0];
                                                    let target_id = target.id;
                                                    let mut records = self.records.write().await;
                                                    records.remove(&target_id);
                                                    drop(records);
                                                    let mut victims = self.tk_victims.write().await;
                                                    // Clear TKs this player committed against others
                                                    for events in victims.values_mut() {
                                                        events.retain(|e| e.attacker_id != target_id);
                                                    }
                                                    ctx.say(&format!("^7TK records cleared for ^2{}", target.name)).await?;
                                                }
                                                _ => {
                                                    ctx.message(&cid_str, &format!("^7Multiple matches for ^3{}^7. Be more specific.", _args)).await?;
                                                }
                                            }
                                        }
                                    } else {
                                        ctx.message(&cid_str, "^7You do not have permission to use this command").await?;
                                    }
                                }
                            }

                            "forgiveprev" | "fp" => {
                                // Show who last TK'd you
                                let victims = self.tk_victims.read().await;
                                if let Some(events) = victims.get(&issuer_id) {
                                    if let Some(last) = events.last() {
                                        let status = if last.forgiven { "^2forgiven" } else { "^1not forgiven" };
                                        ctx.message(&cid_str, &format!("^7Last TK: ^3{} ^7({}^7)", last.attacker_name, status)).await?;
                                    } else {
                                        ctx.message(&cid_str, "^7No one has team killed you").await?;
                                    }
                                } else {
                                    ctx.message(&cid_str, "^7No one has team killed you").await?;
                                }
                            }

                            _ => {}
                        }
                    }
                }
                return Ok(());
            }
        }

        if let EventData::Kill { damage, .. } = &event.data {
                let Some(attacker_id) = event.client_id else {
                    return Ok(());
                };

                let mut records = self.records.write().await;
                let record = records.entry(attacker_id).or_default();

                // Determine if this is a kill or damage event
                if let Some(key) = ctx.event_registry.get_key(event.event_type) {
                    if key == "EVT_CLIENT_KILL_TEAM" {
                        record.team_kills += 1;
                        record.team_damage += damage;

                        // Track victim for forgive system
                        if let Some(victim_id) = event.target_id {
                            let attacker_name = ctx.clients.get_by_cid(&attacker_id.to_string()).await
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| format!("Player {}", attacker_id));

                            let mut victims = self.tk_victims.write().await;
                            victims.entry(victim_id).or_default().push(TkEvent {
                                attacker_id,
                                attacker_name: attacker_name.clone(),
                                forgiven: false,
                            });
                            drop(victims);

                            // Notify victim about forgive option
                            ctx.message(
                                &victim_id.to_string(),
                                &format!("^7{} ^1team killed ^7you! Type ^2!forgive ^7to forgive or stay silent to punish.", attacker_name),
                            ).await?;
                        }

                        info!(
                            attacker = attacker_id,
                            tk_count = record.team_kills,
                            "Team kill recorded"
                        );

                        // Check unforgiven TKs instead of raw count
                        let unforgiven = {
                            drop(records);
                            self.unforgiven_tk_count(attacker_id).await
                        };

                        if unforgiven >= self.max_team_kills {
                            warn!(attacker = attacker_id, unforgiven = unforgiven, "TK limit exceeded (unforgiven)");
                            ctx.message(
                                &attacker_id.to_string(),
                                &format!("^1You have been kicked for {} unforgiven team kills", self.max_team_kills),
                            ).await?;
                            ctx.kick(&attacker_id.to_string(), "Too many team kills").await?;
                        }
                    } else if key == "EVT_CLIENT_DAMAGE_TEAM" {
                        record.team_damage += damage;

                        if record.team_damage >= self.max_team_damage {
                            warn!(attacker = attacker_id, damage = record.team_damage, "Team damage limit exceeded");
                            drop(records);
                            ctx.message(
                                &attacker_id.to_string(),
                                "^1You have been kicked for excessive team damage",
                            ).await?;
                            ctx.kick(&attacker_id.to_string(), "Excessive team damage").await?;
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
        Some(vec![
            "EVT_CLIENT_KILL_TEAM".to_string(),
            "EVT_CLIENT_DAMAGE_TEAM".to_string(),
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
        ])
    }
}
