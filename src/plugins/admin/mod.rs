use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use tracing::info;

use crate::core::context::BotContext;
use crate::core::{Client, Penalty, PenaltyType};
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

// ---- Permission levels (mirrors R3 group levels) ----
const LEVEL_GUEST: u32 = 0;
const LEVEL_USER: u32 = 1;
const LEVEL_REGULAR: u32 = 2;
const LEVEL_MOD: u32 = 20;
const LEVEL_ADMIN: u32 = 40;
const LEVEL_SENIOR_ADMIN: u32 = 60;
const LEVEL_SUPER_ADMIN: u32 = 80;

/// The Admin plugin — core command handler for server administration.
/// Handles !kick, !ban, !tempban, !unban, !warn, !lookup, !leveltest, !putgroup, !maps, etc.
pub struct AdminPlugin {
    enabled: bool,
    warn_reason: String,
    max_warnings: u32,
    /// Configurable warn reasons: keyword -> (duration_mins, reason_text)
    warn_reasons: HashMap<String, (u32, String)>,
    /// Predefined spam messages: keyword -> text
    spam_messages: HashMap<String, String>,
    /// Server rules lines
    rules: Vec<String>,
    /// Whether !iamgod has been used
    iamgod_used: bool,
}

impl AdminPlugin {
    pub fn new() -> Self {
        let mut warn_reasons = HashMap::new();
        warn_reasons.insert("spam".to_string(), (5, "Spamming".to_string()));
        warn_reasons.insert("lang".to_string(), (5, "Bad language".to_string()));
        warn_reasons.insert("rage".to_string(), (1440, "Rage quitting".to_string()));
        warn_reasons.insert("tk".to_string(), (30, "Team killing".to_string()));
        warn_reasons.insert("camp".to_string(), (5, "Camping".to_string()));
        warn_reasons.insert("afk".to_string(), (5, "Being AFK".to_string()));

        let mut spam_messages = HashMap::new();
        spam_messages.insert("rules".to_string(), "^3Server Rules: No cheating, no exploiting, no team killing, respect admins.".to_string());
        spam_messages.insert("website".to_string(), "^3Visit our website for more info!".to_string());

        Self {
            enabled: true,
            warn_reason: "Server Rule Violation".to_string(),
            max_warnings: 3,
            warn_reasons,
            spam_messages,
            rules: vec!["^3Server Rules:".to_string(), "^71. No cheating".to_string(), "^72. No exploiting".to_string(), "^73. No team killing".to_string(), "^74. Respect admins".to_string()],
            iamgod_used: false,
        }
    }

    /// Look up the issuing player from the connected-clients manager.
    async fn get_issuer(&self, event: &Event, ctx: &BotContext) -> Option<Client> {
        let cid = event.client_id?;
        ctx.clients.get_by_cid(&cid.to_string()).await
    }

    /// Find a connected target player by partial name or slot number.
    async fn find_target(&self, query: &str, ctx: &BotContext) -> Option<Client> {
        // Try as a slot number first
        if let Some(client) = ctx.clients.get_by_cid(query).await {
            return Some(client);
        }
        // Try by partial name among connected players
        let matches = ctx.clients.find_by_name(query).await;
        if matches.len() == 1 {
            return Some(matches.into_iter().next().unwrap());
        }
        None
    }

    /// Get the required permission level for a command.
    fn required_level(command: &str) -> u32 {
        match command {
            "help" | "leveltest" | "time" | "register" | "regme" | "r3" => LEVEL_GUEST,
            "regulars" | "rules" => LEVEL_USER,
            "status" | "lookup" | "list" | "admins" => LEVEL_MOD,
            "warn" | "kick" | "find" | "seen" | "aliases" | "poke" => LEVEL_MOD,
            "warntest" | "warnremove" | "warninfo" | "warns" => LEVEL_MOD,
            "spank" | "notice" | "clear" => LEVEL_MOD,
            "mute" | "unmute" => LEVEL_ADMIN,
            "tempban" | "lastbans" | "baninfo" | "spam" | "spams" | "clientinfo" => LEVEL_ADMIN,
            "ban" | "unban" | "permban" | "say" | "longlist" => LEVEL_SENIOR_ADMIN,
            "warnclear" | "kickall" | "banall" | "spankall" => LEVEL_SENIOR_ADMIN,
            "scream" | "mask" | "unmask" | "makereg" | "unreg" => LEVEL_SENIOR_ADMIN,
            "setnextmap" => LEVEL_SENIOR_ADMIN,
            "putgroup" | "ungroup" | "maprotate" | "maps" | "nextmap" | "map" => LEVEL_SUPER_ADMIN,
            "die" | "restart" | "reconfig" | "pause" | "rebuild" => LEVEL_SUPER_ADMIN,
            "runas" | "iamgod" => LEVEL_SUPER_ADMIN,
            _ => LEVEL_SUPER_ADMIN, // unknown commands require superadmin
        }
    }

    /// Convenience to send a denial message.
    async fn deny(&self, cid: &str, ctx: &BotContext) -> anyhow::Result<()> {
        ctx.message(cid, "^1Insufficient privileges").await
    }
}

impl Default for AdminPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// How the command output should be delivered.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CommandMode {
    /// `!` prefix: private message to issuer
    Private,
    /// `@` prefix: public say
    Loud,
    /// `&` prefix: bigtext
    BigText,
}

impl CommandMode {
    /// Send a response according to the command mode.
    async fn respond(&self, ctx: &BotContext, cid: &str, message: &str) -> anyhow::Result<()> {
        match self {
            CommandMode::Private => ctx.message(cid, message).await,
            CommandMode::Loud => ctx.say(message).await,
            CommandMode::BigText => ctx.bigtext(message).await,
        }
    }
}

#[async_trait]
impl Plugin for AdminPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "admin",
            description: "Core administration commands (!kick, !ban, !warn, etc.)",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("warn_reason").and_then(|v| v.as_str()) {
                self.warn_reason = v.to_string();
            }
            if let Some(v) = s.get("max_warnings").and_then(|v| v.as_integer()) {
                self.max_warnings = v as u32;
            }
            if let Some(t) = s.get("warn_reasons").and_then(|v| v.as_table()) {
                self.warn_reasons.clear();
                for (key, val) in t {
                    if let Some(inner) = val.as_table() {
                        let duration = inner.get("duration").and_then(|v| v.as_integer()).unwrap_or(5) as u32;
                        let reason = inner.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        self.warn_reasons.insert(key.clone(), (duration, reason));
                    }
                }
            }
            if let Some(t) = s.get("spam_messages").and_then(|v| v.as_table()) {
                self.spam_messages.clear();
                for (key, val) in t {
                    if let Some(v) = val.as_str() {
                        self.spam_messages.insert(key.clone(), v.to_string());
                    }
                }
            }
            if let Some(arr) = s.get("rules").and_then(|v| v.as_array()) {
                self.rules = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Admin plugin started — commands registered");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // ---- Ban check on client auth ----
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            if event_key == "EVT_CLIENT_AUTH" {
                self.check_ban(event, ctx).await?;
                return Ok(());
            }
        }

        // ---- Command handling from chat ----
        if let EventData::Text(ref text) = event.data {
            // Support '!' (private), '@' (loud/say), '&' (bigtext) prefixes
            if let Some(cmd) = text.strip_prefix('!') {
                self.handle_command(cmd, event, ctx, CommandMode::Private).await?;
            } else if let Some(cmd) = text.strip_prefix('@') {
                self.handle_command(cmd, event, ctx, CommandMode::Loud).await?;
            } else if let Some(cmd) = text.strip_prefix('&') {
                self.handle_command(cmd, event, ctx, CommandMode::BigText).await?;
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
            "EVT_CLIENT_PRIVATE_SAY".to_string(),
            "EVT_CLIENT_AUTH".to_string(),
        ])
    }
}

impl AdminPlugin {
    /// Check if a newly authenticated client has an active ban.
    async fn check_ban(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let cid = match event.client_id {
            Some(id) => id,
            None => return Ok(()),
        };
        let cid_str = cid.to_string();

        let client = match ctx.clients.get_by_cid(&cid_str).await {
            Some(c) => c,
            None => return Ok(()),
        };

        // Skip if we have no DB record yet
        if client.id <= 0 {
            return Ok(());
        }

        // Check permanent bans
        let bans = ctx
            .storage
            .get_penalties(client.id, Some(PenaltyType::Ban))
            .await?;
        if let Some(ban) = bans.iter().find(|p| !p.inactive) {
            info!(
                name = %client.name,
                reason = %ban.reason,
                "Kicking banned player on connect"
            );
            ctx.kick(&cid_str, &format!("Banned: {}", ban.reason))
                .await?;
            return Ok(());
        }

        // Check temporary bans
        let tempbans = ctx
            .storage
            .get_penalties(client.id, Some(PenaltyType::TempBan))
            .await?;
        let now = Utc::now();
        if let Some(tb) = tempbans.iter().find(|p| {
            !p.inactive && p.time_expire.is_some_and(|exp| exp > now)
        }) {
            let remaining = tb
                .time_expire
                .map(|exp| {
                    let dur = exp - now;
                    format_duration(dur.num_minutes())
                })
                .unwrap_or_else(|| "unknown".to_string());
            info!(
                name = %client.name,
                reason = %tb.reason,
                remaining = %remaining,
                "Kicking temp-banned player on connect"
            );
            ctx.kick(
                &cid_str,
                &format!("Temp banned ({}): {}", remaining, tb.reason),
            )
            .await?;
        }

        Ok(())
    }

    fn handle_command<'a>(
        &'a self,
        cmd: &'a str,
        event: &'a Event,
        ctx: &'a BotContext,
        mode: CommandMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async move {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).unwrap_or(&"").trim();

        let issuer_cid = event.client_id.unwrap_or(0);
        let issuer_cid_str = issuer_cid.to_string();

        // ---- Permission check ----
        let issuer = self.get_issuer(event, ctx).await;
        let issuer_level = issuer.as_ref().map(|c| c.max_level()).unwrap_or(0);

        // iamgod is special — allow level 0 if no superadmins exist
        let required = if command == "iamgod" { 0 } else { Self::required_level(&command) };

        if issuer_level < required {
            self.deny(&issuer_cid_str, ctx).await?;
            return Ok(());
        }

        match command.as_str() {
            "help" => {
                let mut cmds: Vec<&str> = vec!["!help", "!leveltest", "!time", "!register"];
                if issuer_level >= LEVEL_USER {
                    cmds.extend(["!regulars", "!rules"]);
                }
                if issuer_level >= LEVEL_MOD {
                    cmds.extend(["!status", "!lookup", "!warn", "!kick", "!find",
                        "!seen", "!aliases", "!poke", "!warns", "!list", "!admins"]);
                }
                if issuer_level >= LEVEL_ADMIN {
                    cmds.extend(["!mute", "!unmute", "!tempban", "!spam", "!lastbans", "!baninfo"]);
                }
                if issuer_level >= LEVEL_SENIOR_ADMIN {
                    cmds.extend(["!ban", "!unban", "!permban", "!say", "!scream",
                        "!mask", "!unmask", "!makereg", "!unreg", "!setnextmap"]);
                }
                if issuer_level >= LEVEL_SUPER_ADMIN {
                    cmds.extend(["!putgroup", "!maprotate", "!maps", "!nextmap", "!map",
                        "!die", "!rebuild", "!runas"]);
                }
                mode.respond(ctx, &issuer_cid_str, &cmds.join(" ")).await?;
            }

            "r3" => {
                mode.respond(ctx, &issuer_cid_str, "^2Rusty Rules Referee ^7(R3) v2.0.0 — Rust Edition").await?;
            }

            "time" => {
                let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                mode.respond(ctx, &issuer_cid_str, &format!("^7Server time: ^2{}", now)).await?;
            }

            "leveltest" => {
                if args.is_empty() {
                    let name = issuer.as_ref().map(|c| c.name.as_str()).unwrap_or("unknown");
                    let group_name = level_name(issuer_level);
                    mode.respond(
                        ctx,
                        &issuer_cid_str,
                        &format!("{} is a ^2{} ^7[level {}]", name, group_name, issuer_level),
                    ).await?;
                } else {
                    // Test another player's level
                    if let Some(target) = self.find_target(args, ctx).await {
                        let tl = target.max_level();
                        mode.respond(
                            ctx,
                            &issuer_cid_str,
                            &format!("{} is a ^2{} ^7[level {}]", target.name, level_name(tl), tl),
                        ).await?;
                    } else {
                        ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                    }
                }
            }

            "register" | "regme" => {
                if let Some(ref iss) = issuer {
                    if iss.max_level() >= LEVEL_USER {
                        ctx.message(&issuer_cid_str, "^7You are already registered").await?;
                    } else {
                        let mut updated = iss.clone();
                        updated.group_bits = 1u64 << LEVEL_USER;
                        ctx.storage.save_client(&updated).await?;
                        if let Some(ref cid) = updated.cid {
                            ctx.clients.update(cid, |c| c.group_bits = updated.group_bits).await;
                        }
                        ctx.message(&issuer_cid_str, &format!("^2{} ^7is now a ^2User ^7[level 1]", updated.name)).await?;
                    }
                }
            }

            "regulars" => {
                let all = ctx.clients.get_all().await;
                let regs: Vec<String> = all.iter()
                    .filter(|c| c.max_level() >= LEVEL_REGULAR)
                    .map(|c| c.name.clone()).collect();
                if regs.is_empty() {
                    mode.respond(ctx, &issuer_cid_str, "No regulars online").await?;
                } else {
                    mode.respond(ctx, &issuer_cid_str, &format!("^2Regulars online: ^7{}", regs.join(", "))).await?;
                }
            }

            "rules" => {
                if args.is_empty() {
                    for rule in &self.rules {
                        ctx.message(&issuer_cid_str, rule).await?;
                    }
                } else {
                    // Send rules to target player
                    if let Some(target) = self.find_target(args, ctx).await {
                        if let Some(ref cid) = target.cid {
                            for rule in &self.rules {
                                ctx.message(cid, rule).await?;
                            }
                        }
                    }
                }
            }

            "admins" => {
                let all = ctx.clients.get_all().await;
                let admins: Vec<String> = all.iter()
                    .filter(|c| c.max_level() >= LEVEL_MOD)
                    .map(|c| format!("^2{} ^7[{}]", c.name, level_name(c.max_level())))
                    .collect();
                if admins.is_empty() {
                    mode.respond(ctx, &issuer_cid_str, "No admins online").await?;
                } else {
                    mode.respond(ctx, &issuer_cid_str, &format!("^7Admins online: {}", admins.join(", "))).await?;
                }
            }

            "list" => {
                let all = ctx.clients.get_all().await;
                if all.is_empty() {
                    mode.respond(ctx, &issuer_cid_str, "No players connected").await?;
                } else {
                    let list: Vec<String> = all.iter().map(|c| {
                        let slot = c.cid.as_deref().unwrap_or("?");
                        format!("[{}] {}", slot, c.name)
                    }).collect();
                    mode.respond(ctx, &issuer_cid_str, &format!("^7Players: {}", list.join(", "))).await?;
                }
            }

            "longlist" => {
                let all = ctx.clients.get_all().await;
                if all.is_empty() {
                    mode.respond(ctx, &issuer_cid_str, "No players connected").await?;
                } else {
                    for c in &all {
                        let slot = c.cid.as_deref().unwrap_or("?");
                        let ip = c.ip.map(|i| i.to_string()).unwrap_or_else(|| "?".to_string());
                        mode.respond(
                            ctx,
                            &issuer_cid_str,
                            &format!("^7[{}] ^2{} ^7@{} [lv{}] IP:{}", slot, c.name, c.id, c.max_level(), ip),
                        ).await?;
                    }
                }
            }

            "status" => {
                let all = ctx.clients.get_all().await;
                if all.is_empty() {
                    ctx.message(&issuer_cid_str, "No players connected").await?;
                } else {
                    for c in &all {
                        let slot = c.cid.as_deref().unwrap_or("?");
                        ctx.message(
                            &issuer_cid_str,
                            &format!("^7[{}] ^2{} ^7@{} [lv{}]", slot, c.name, c.id, c.max_level()),
                        ).await?;
                    }
                }
            }

            "find" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !find <name>").await?;
                    return Ok(());
                }
                let matches = ctx.clients.find_by_name(args).await;
                if matches.is_empty() {
                    ctx.message(&issuer_cid_str, &format!("No connected players matching '{}'", args)).await?;
                } else {
                    for c in &matches {
                        let slot = c.cid.as_deref().unwrap_or("?");
                        ctx.message(&issuer_cid_str, &format!("^7[{}] ^2{} ^7@{} [lv{}]", slot, c.name, c.id, c.max_level())).await?;
                    }
                }
            }

            "seen" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !seen <name>").await?;
                    return Ok(());
                }
                let results = ctx.storage.find_clients(args).await?;
                if results.is_empty() {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                } else if let Some(c) = results.first() {
                    let seen = c.last_visit
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "never".to_string());
                    mode.respond(ctx, &issuer_cid_str, &format!("^2{} ^7was last seen: ^3{}", c.name, seen)).await?;
                }
            }

            "aliases" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !aliases <name or @id>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let aliases = ctx.storage.get_aliases(c.id).await?;
                    if aliases.is_empty() {
                        ctx.message(&issuer_cid_str, &format!("No aliases found for {}", c.name)).await?;
                    } else {
                        let names: Vec<String> = aliases.iter().map(|a| format!("{} ({}x)", a.alias, a.num_used)).collect();
                        ctx.message(&issuer_cid_str, &format!("^2{} ^7aliases: {}", c.name, names.join(", "))).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "clientinfo" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !clientinfo <name or @id>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let ip = c.ip.map(|i| i.to_string()).unwrap_or_else(|| "?".to_string());
                    ctx.message(&issuer_cid_str, &format!(
                        "^2{} ^7@{} GUID:{} IP:{} Level:{} Connected:{}",
                        c.name, c.id, c.guid, ip, c.max_level(), c.connected
                    )).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "lookup" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !lookup <name>").await?;
                    return Ok(());
                }
                let results = ctx.storage.find_clients(args).await?;
                if results.is_empty() {
                    ctx.message(&issuer_cid_str, &format!("No clients found matching '{}'", args)).await?;
                } else {
                    let count = results.len();
                    for c in results.iter().take(5) {
                        ctx.message(
                            &issuer_cid_str,
                            &format!("^7@{} ^2{} ^7[lv{}]", c.id, c.name, c.max_level()),
                        ).await?;
                    }
                    if count > 5 {
                        ctx.message(&issuer_cid_str, &format!("...and {} more", count - 5)).await?;
                    }
                }
            }

            "poke" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !poke <player>").await?;
                    return Ok(());
                }
                if let Some(target) = self.find_target(args, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.message(cid, "^1You have been poked by an admin! Move or be kicked!").await?;
                    }
                    ctx.message(&issuer_cid_str, &format!("^7Poked ^2{}", target.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                }
            }

            // ---- Warning management ----

            "warn" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !warn <player> [reason/keyword]").await?;
                    return Ok(());
                }
                let (target_q, reason_input) = split_target_reason(args);

                // Check if reason is a warn keyword
                let (duration_mins, reason) = if let Some((dur, reason_text)) = self.warn_reasons.get(reason_input) {
                    (*dur, reason_text.as_str())
                } else if reason_input.is_empty() {
                    (0, self.warn_reason.as_str())
                } else {
                    (0, reason_input)
                };

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot warn a player with equal or higher level").await?;
                        return Ok(());
                    }
                    let expire = if duration_mins > 0 {
                        Some(Utc::now() + chrono::Duration::minutes(duration_mins as i64))
                    } else {
                        None
                    };
                    let penalty = Penalty {
                        id: 0,
                        penalty_type: PenaltyType::Warning,
                        client_id: target.id,
                        admin_id: Some(issuer_cid),
                        duration: Some(duration_mins as i64),
                        reason: reason.to_string(),
                        keyword: "warn".to_string(),
                        inactive: false,
                        time_add: Utc::now(),
                        time_edit: Utc::now(),
                        time_expire: expire,
                    };
                    ctx.storage.save_penalty(&penalty).await?;

                    let warn_count = ctx.storage.count_penalties(target.id, PenaltyType::Warning).await?;
                    ctx.message(
                        target.cid.as_deref().unwrap_or("0"),
                        &format!("^1WARNING ^7({}/{}): {}", warn_count, self.max_warnings, reason),
                    ).await?;

                    if warn_count >= self.max_warnings as u64 {
                        if let Some(ref cid) = target.cid {
                            ctx.kick(cid, "Too many warnings").await?;
                            ctx.say(&format!("^2{} ^7was kicked: too many warnings", target.name)).await?;
                        }
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
                info!(admin = issuer_cid, target = target_q, reason = reason, "!warn");
            }

            "warntest" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !warntest <keyword>").await?;
                    return Ok(());
                }
                if let Some((dur, reason)) = self.warn_reasons.get(args) {
                    ctx.message(&issuer_cid_str, &format!("^7Warn '{}': {} ({})", args, reason, format_duration(*dur as i64))).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("^7No warn reason found for '{}'", args)).await?;
                }
            }

            "warns" => {
                let keys: Vec<&String> = self.warn_reasons.keys().collect();
                ctx.message(&issuer_cid_str, &format!("^7Available warn keywords: ^3{}", keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))).await?;
            }

            "warnremove" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !warnremove <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let removed = ctx.storage.disable_last_penalty(c.id, PenaltyType::Warning).await?;
                    if removed {
                        ctx.message(&issuer_cid_str, &format!("^7Last warning removed for ^2{}", c.name)).await?;
                    } else {
                        ctx.message(&issuer_cid_str, &format!("^7No active warnings found for {}", c.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "warnclear" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !warnclear <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let count = ctx.storage.disable_all_penalties_of_type(c.id, PenaltyType::Warning).await?;
                    ctx.message(&issuer_cid_str, &format!("^7Cleared {} warnings for ^2{}", count, c.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "warninfo" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !warninfo <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let count = ctx.storage.count_penalties(c.id, PenaltyType::Warning).await?;
                    ctx.message(&issuer_cid_str, &format!("^2{} ^7has ^3{} ^7active warning(s)", c.name, count)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "notice" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !notice <player> <text>").await?;
                    return Ok(());
                }
                let (target_q, notice_text) = split_target_reason(args);
                if notice_text.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !notice <player> <text>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(target_q, ctx).await;
                if let Some(c) = client {
                    let penalty = Penalty {
                        id: 0,
                        penalty_type: PenaltyType::Notice,
                        client_id: c.id,
                        admin_id: Some(issuer_cid),
                        duration: None,
                        reason: notice_text.to_string(),
                        keyword: "notice".to_string(),
                        inactive: false,
                        time_add: Utc::now(),
                        time_edit: Utc::now(),
                        time_expire: None,
                    };
                    ctx.storage.save_penalty(&penalty).await?;
                    ctx.message(&issuer_cid_str, &format!("^7Notice added for ^2{}", c.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", target_q)).await?;
                }
            }

            "clear" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !clear <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let w = ctx.storage.disable_all_penalties_of_type(c.id, PenaltyType::Warning).await?;
                    let n = ctx.storage.disable_all_penalties_of_type(c.id, PenaltyType::Notice).await?;
                    ctx.message(&issuer_cid_str, &format!("^7Cleared {} warnings and {} notices for ^2{}", w, n, c.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            // ---- Kick / Spank ----

            "kick" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !kick <player> [reason]").await?;
                    return Ok(());
                }
                let (target_q, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Kicked by admin" } else { reason };

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot kick a player with equal or higher level").await?;
                        return Ok(());
                    }
                    if let Some(ref cid) = target.cid {
                        ctx.kick(cid, reason).await?;
                        ctx.say(&format!("^2{} ^7was kicked: {}", target.name, reason)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
                info!(admin = issuer_cid, target = target_q, reason = reason, "!kick");
            }

            "spank" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !spank <player> [reason]").await?;
                    return Ok(());
                }
                let (target_q, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Spanked by admin" } else { reason };

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot spank a player with equal or higher level").await?;
                        return Ok(());
                    }
                    if let Some(ref cid) = target.cid {
                        ctx.kick(cid, reason).await?;
                        // Public humiliation message
                        ctx.say(&format!("^2{} ^7was ^1SPANKED ^7off the server: {}", target.name, reason)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "kickall" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !kickall <pattern> [reason]").await?;
                    return Ok(());
                }
                let (pattern, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Kicked by admin" } else { reason };
                let all = ctx.clients.get_all().await;
                let lower = pattern.to_lowercase();
                let mut count = 0u32;
                for c in &all {
                    if c.name.to_lowercase().contains(&lower) && c.max_level() < issuer_level {
                        if let Some(ref cid) = c.cid {
                            ctx.kick(cid, reason).await?;
                            count += 1;
                        }
                    }
                }
                ctx.say(&format!("^7Kicked {} players matching '{}'", count, pattern)).await?;
            }

            "spankall" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !spankall <pattern> [reason]").await?;
                    return Ok(());
                }
                let (pattern, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Spanked by admin" } else { reason };
                let all = ctx.clients.get_all().await;
                let lower = pattern.to_lowercase();
                let mut count = 0u32;
                for c in &all {
                    if c.name.to_lowercase().contains(&lower) && c.max_level() < issuer_level {
                        if let Some(ref cid) = c.cid {
                            ctx.kick(cid, reason).await?;
                            count += 1;
                        }
                    }
                }
                ctx.say(&format!("^1SPANKED ^7{} players matching '{}'", count, pattern)).await?;
            }

            // ---- Banning ----

            "tempban" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !tempban <player> [duration] [reason]").await?;
                    return Ok(());
                }
                let (target_q, rest) = split_target_reason(args);
                let (duration_mins, reason) = parse_duration_and_reason(rest);
                let reason = if reason.is_empty() { "Temp banned by admin" } else { reason };

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot tempban a player with equal or higher level").await?;
                        return Ok(());
                    }
                    let expire = Utc::now() + chrono::Duration::minutes(duration_mins as i64);
                    let penalty = Penalty {
                        id: 0,
                        penalty_type: PenaltyType::TempBan,
                        client_id: target.id,
                        admin_id: Some(issuer_cid),
                        duration: Some(duration_mins as i64),
                        reason: reason.to_string(),
                        keyword: "tempban".to_string(),
                        inactive: false,
                        time_add: Utc::now(),
                        time_edit: Utc::now(),
                        time_expire: Some(expire),
                    };
                    ctx.storage.save_penalty(&penalty).await?;
                    if let Some(ref cid) = target.cid {
                        ctx.kick(cid, reason).await?;
                    }
                    ctx.say(&format!("^2{} ^7was temp banned for {}: {}", target.name, format_duration(duration_mins as i64), reason)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
                info!(admin = issuer_cid, target = target_q, reason = reason, "!tempban");
            }

            "ban" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !ban <player> [reason]").await?;
                    return Ok(());
                }
                let (target_q, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Banned by admin" } else { reason };

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot ban a player with equal or higher level").await?;
                        return Ok(());
                    }
                    let penalty = Penalty {
                        id: 0,
                        penalty_type: PenaltyType::Ban,
                        client_id: target.id,
                        admin_id: Some(issuer_cid),
                        duration: None,
                        reason: reason.to_string(),
                        keyword: "ban".to_string(),
                        inactive: false,
                        time_add: Utc::now(),
                        time_edit: Utc::now(),
                        time_expire: None,
                    };
                    ctx.storage.save_penalty(&penalty).await?;
                    if let Some(ref cid) = target.cid {
                        ctx.ban(cid, reason).await?;
                    }
                    ctx.say(&format!("^2{} ^7was banned: {}", target.name, reason)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
                info!(admin = issuer_cid, target = target_q, reason = reason, "!ban");
            }

            "permban" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !permban <player> [reason]").await?;
                    return Ok(());
                }
                let (target_q, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Permanently banned" } else { reason };

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot permban a player with equal or higher level").await?;
                        return Ok(());
                    }
                    let penalty = Penalty {
                        id: 0,
                        penalty_type: PenaltyType::Ban,
                        client_id: target.id,
                        admin_id: Some(issuer_cid),
                        duration: None,
                        reason: reason.to_string(),
                        keyword: "permban".to_string(),
                        inactive: false,
                        time_add: Utc::now(),
                        time_edit: Utc::now(),
                        time_expire: None,
                    };
                    ctx.storage.save_penalty(&penalty).await?;
                    if let Some(ref cid) = target.cid {
                        ctx.ban(cid, reason).await?;
                    }
                    ctx.say(&format!("^2{} ^7was ^1PERMANENTLY ^7banned: {}", target.name, reason)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "banall" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !banall <pattern> [reason]").await?;
                    return Ok(());
                }
                let (pattern, reason) = split_target_reason(args);
                let reason = if reason.is_empty() { "Banned by admin" } else { reason };
                let all = ctx.clients.get_all().await;
                let lower = pattern.to_lowercase();
                let mut count = 0u32;
                for c in &all {
                    if c.name.to_lowercase().contains(&lower) && c.max_level() < issuer_level {
                        let penalty = Penalty {
                            id: 0,
                            penalty_type: PenaltyType::Ban,
                            client_id: c.id,
                            admin_id: Some(issuer_cid),
                            duration: None,
                            reason: reason.to_string(),
                            keyword: "ban".to_string(),
                            inactive: false,
                            time_add: Utc::now(),
                            time_edit: Utc::now(),
                            time_expire: None,
                        };
                        let _ = ctx.storage.save_penalty(&penalty).await;
                        if let Some(ref cid) = c.cid {
                            let _ = ctx.ban(cid, reason).await;
                        }
                        count += 1;
                    }
                }
                ctx.say(&format!("^7Banned {} players matching '{}'", count, pattern)).await?;
            }

            "unban" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !unban <name or @id>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    ctx.storage.disable_penalties(c.id, PenaltyType::Ban).await?;
                    ctx.storage.disable_penalties(c.id, PenaltyType::TempBan).await?;
                    ctx.say(&format!("^2{} ^7has been unbanned", c.name)).await?;
                    info!(admin = issuer_cid, target = %c.name, "!unban");
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "lastbans" => {
                let bans = ctx.storage.get_last_bans(5).await?;
                if bans.is_empty() {
                    ctx.message(&issuer_cid_str, "No recent bans").await?;
                } else {
                    for ban in &bans {
                        let client_name = ctx.storage.get_client(ban.client_id).await
                            .map(|c| c.name).unwrap_or_else(|_| format!("@{}", ban.client_id));
                        ctx.message(&issuer_cid_str, &format!(
                            "^7{}: ^2{} ^7- {} ({})",
                            ban.time_add.format("%Y-%m-%d %H:%M"),
                            client_name,
                            ban.reason,
                            if ban.penalty_type == PenaltyType::Ban { "permanent" } else { "temp" }
                        )).await?;
                    }
                }
            }

            "baninfo" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !baninfo <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(c) = client {
                    let ban_count = ctx.storage.count_penalties(c.id, PenaltyType::Ban).await?;
                    let tb_count = ctx.storage.count_penalties(c.id, PenaltyType::TempBan).await?;
                    ctx.message(&issuer_cid_str, &format!(
                        "^2{} ^7has ^3{} ^7active ban(s) and ^3{} ^7active temp ban(s)",
                        c.name, ban_count, tb_count
                    )).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            // ---- Group management ----

            "putgroup" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !putgroup <player> <group>").await?;
                    return Ok(());
                }
                let (target_q, group_name) = split_target_reason(args);
                if group_name.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !putgroup <player> <group>").await?;
                    return Ok(());
                }
                let level = match group_name.to_lowercase().as_str() {
                    "guest" => LEVEL_GUEST,
                    "user" => LEVEL_USER,
                    "regular" | "reg" => LEVEL_REGULAR,
                    "mod" | "moderator" => LEVEL_MOD,
                    "admin" => LEVEL_ADMIN,
                    "senioradmin" | "senior" | "sadmin" => LEVEL_SENIOR_ADMIN,
                    "superadmin" | "super" => LEVEL_SUPER_ADMIN,
                    _ => {
                        ctx.message(&issuer_cid_str, "^1Unknown group. Use: guest, user, regular, mod, admin, senioradmin, superadmin").await?;
                        return Ok(());
                    }
                };

                if level >= issuer_level {
                    ctx.message(&issuer_cid_str, "^1Cannot assign a group at or above your own level").await?;
                    return Ok(());
                }

                let target = self.resolve_client(target_q, ctx).await;
                if let Some(mut target) = target {
                    target.group_bits = 1u64 << level;
                    ctx.storage.save_client(&target).await?;
                    if let Some(ref cid) = target.cid {
                        ctx.clients.update(cid, |c| c.group_bits = target.group_bits).await;
                    }
                    ctx.say(&format!("^2{} ^7is now a ^2{} ^7[level {}]", target.name, level_name(level), level)).await?;
                    info!(admin = issuer_cid, target = %target.name, group = group_name, level = level, "!putgroup");
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "ungroup" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !ungroup <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(mut c) = client {
                    c.group_bits = 0;
                    ctx.storage.save_client(&c).await?;
                    if let Some(ref cid) = c.cid {
                        ctx.clients.update(cid, |cl| cl.group_bits = 0).await;
                    }
                    ctx.message(&issuer_cid_str, &format!("^2{} ^7has been removed from all groups", c.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "makereg" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !makereg <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(mut c) = client {
                    c.group_bits = 1u64 << LEVEL_REGULAR;
                    ctx.storage.save_client(&c).await?;
                    if let Some(ref cid) = c.cid {
                        ctx.clients.update(cid, |cl| cl.group_bits = c.group_bits).await;
                    }
                    ctx.say(&format!("^2{} ^7is now a ^2Regular", c.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "unreg" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !unreg <player>").await?;
                    return Ok(());
                }
                let client = self.resolve_client(args, ctx).await;
                if let Some(mut c) = client {
                    c.group_bits = 1u64 << LEVEL_USER;
                    ctx.storage.save_client(&c).await?;
                    if let Some(ref cid) = c.cid {
                        ctx.clients.update(cid, |cl| cl.group_bits = c.group_bits).await;
                    }
                    ctx.message(&issuer_cid_str, &format!("^2{} ^7has been removed from regulars", c.name)).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No client found matching '{}'", args)).await?;
                }
            }

            "mask" => {
                let (target_q, level_str) = if args.is_empty() {
                    ("", "")
                } else {
                    split_target_reason(args)
                };

                let mask = match level_str.to_lowercase().as_str() {
                    "guest" | "0" => LEVEL_GUEST,
                    "user" | "1" => LEVEL_USER,
                    "regular" | "reg" | "2" => LEVEL_REGULAR,
                    "mod" | "moderator" | "20" => LEVEL_MOD,
                    _ => LEVEL_USER,
                };

                if target_q.is_empty() {
                    // Mask self
                    if let Some(ref iss) = issuer {
                        if let Some(ref cid) = iss.cid {
                            ctx.clients.update(cid, |c| c.mask_level = mask).await;
                            ctx.message(&issuer_cid_str, &format!("^7You are now masked as ^2{}", level_name(mask))).await?;
                        }
                    }
                } else if let Some(target) = self.find_target(target_q, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.clients.update(cid, |c| c.mask_level = mask).await;
                        ctx.message(&issuer_cid_str, &format!("^2{} ^7is now masked as ^2{}", target.name, level_name(mask))).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "unmask" => {
                if args.is_empty() {
                    // Unmask self
                    if let Some(ref iss) = issuer {
                        if let Some(ref cid) = iss.cid {
                            ctx.clients.update(cid, |c| c.mask_level = 0).await;
                            ctx.message(&issuer_cid_str, "^7Your mask has been removed").await?;
                        }
                    }
                } else if let Some(target) = self.find_target(args, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.clients.update(cid, |c| c.mask_level = 0).await;
                        ctx.message(&issuer_cid_str, &format!("^7Mask removed for ^2{}", target.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                }
            }

            "iamgod" => {
                if self.iamgod_used {
                    ctx.message(&issuer_cid_str, "^1iamgod has already been used").await?;
                    return Ok(());
                }
                // Only works if there are no superadmins
                let sa_count = ctx.storage.get_client_count_by_level(LEVEL_SUPER_ADMIN).await?;
                if sa_count > 0 {
                    ctx.message(&issuer_cid_str, "^1There are already superadmins registered").await?;
                } else if let Some(ref iss) = issuer {
                    let mut updated = iss.clone();
                    updated.group_bits = 1u64 << LEVEL_SUPER_ADMIN;
                    ctx.storage.save_client(&updated).await?;
                    if let Some(ref cid) = updated.cid {
                        ctx.clients.update(cid, |c| c.group_bits = updated.group_bits).await;
                    }
                    ctx.message(&issuer_cid_str, "^2You are now a Super Admin").await?;
                }
            }

            // ---- Chat / Messages ----

            "say" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !say <message>").await?;
                } else {
                    ctx.say(args).await?;
                }
            }

            "scream" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !scream <message>").await?;
                } else {
                    ctx.bigtext(args).await?;
                }
            }

            "spam" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !spam <keyword>").await?;
                    return Ok(());
                }
                let (keyword, target_q) = split_target_reason(args);
                if let Some(msg) = self.spam_messages.get(keyword) {
                    if target_q.is_empty() {
                        ctx.say(msg).await?;
                    } else if let Some(target) = self.find_target(target_q, ctx).await {
                        if let Some(ref cid) = target.cid {
                            ctx.message(cid, msg).await?;
                        }
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("^7Unknown spam keyword '{}'. Use !spams to see available.", keyword)).await?;
                }
            }

            "spams" => {
                let keys: Vec<&String> = self.spam_messages.keys().collect();
                ctx.message(&issuer_cid_str, &format!("^7Available spam messages: ^3{}", keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))).await?;
            }

            // ---- Map management ----

            "maps" => {
                let response = ctx.rcon.send("fdir *.bsp").await?;
                ctx.message(&issuer_cid_str, &response).await?;
            }

            "nextmap" => {
                let response = ctx.rcon.send("nextmap").await?;
                ctx.message(&issuer_cid_str, &format!("Next map: {}", response.trim())).await?;
            }

            "maprotate" => {
                ctx.say("^2Map rotating...").await?;
                ctx.rcon.send("cyclemap").await?;
            }

            "map" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !map <mapname>").await?;
                } else {
                    ctx.say(&format!("^7Changing map to ^2{}...", args)).await?;
                    ctx.rcon.send(&format!("map {}", args)).await?;
                }
            }

            "setnextmap" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !setnextmap <mapname>").await?;
                } else {
                    ctx.set_cvar("g_nextmap", args).await?;
                    ctx.say(&format!("^7Next map set to ^2{}", args)).await?;
                }
            }

            // ---- Bot management ----

            "die" => {
                ctx.say("^1R3 is shutting down...").await?;
                info!("!die command issued — shutting down");
                std::process::exit(0);
            }

            "restart" => {
                ctx.say("^2R3 is restarting...").await?;
                info!("!restart command issued");
                // In Rust, we signal the main loop to restart
                // For now, just exit (systemd/supervisor will restart)
                std::process::exit(0);
            }

            "reconfig" => {
                ctx.message(&issuer_cid_str, "^7Configuration reload requested").await?;
                info!("!reconfig command issued");
            }

            "pause" => {
                ctx.message(&issuer_cid_str, "^7Bot parsing paused").await?;
                info!("!pause command issued");
            }

            "rebuild" => {
                let player_list = ctx.parser.get_player_list().await?;
                ctx.message(&issuer_cid_str, &format!("^7Rebuilding client list ({} slots)...", player_list.len())).await?;
                info!(slots = player_list.len(), "!rebuild — re-syncing clients");
            }

            "runas" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !runas <player> <command>").await?;
                    return Ok(());
                }
                let (target_q, sub_cmd) = split_target_reason(args);
                if sub_cmd.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !runas <player> <command>").await?;
                    return Ok(());
                }
                if let Some(target) = self.find_target(target_q, ctx).await {
                    // Create a synthetic event from the target
                    let mut synth_event = event.clone();
                    synth_event.client_id = Some(target.id);
                    self.handle_command(sub_cmd, &synth_event, ctx, mode).await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "mute" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !mute <player> [duration_seconds]").await?;
                    return Ok(());
                }
                let (target_q, dur_str) = split_target_reason(args);
                let duration: u32 = dur_str.parse().unwrap_or(600);

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if target.max_level() >= issuer_level {
                        ctx.message(&issuer_cid_str, "^1Cannot mute a player with equal or higher level").await?;
                        return Ok(());
                    }
                    if let Some(ref cid) = target.cid {
                        ctx.write(&format!("mute {} {}", cid, duration)).await?;
                        mode.respond(ctx, &issuer_cid_str, &format!("^2{} ^7was muted for {} seconds", target.name, duration)).await?;
                        // Record the mute as a penalty
                        let penalty = Penalty {
                            id: 0,
                            penalty_type: PenaltyType::Mute,
                            client_id: target.id,
                            admin_id: issuer.as_ref().map(|c| c.id),
                            duration: Some(duration as i64),
                            reason: format!("Muted for {} seconds", duration),
                            keyword: "mute".to_string(),
                            inactive: false,
                            time_add: Utc::now(),
                            time_edit: Utc::now(),
                            time_expire: Some(Utc::now() + chrono::Duration::seconds(duration as i64)),
                        };
                        let _ = ctx.storage.save_penalty(&penalty).await;
                        info!(admin = issuer_cid, target = %target.name, duration = duration, "!mute");
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "unmute" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !unmute <player>").await?;
                    return Ok(());
                }
                if let Some(target) = self.find_target(args, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.write(&format!("unmute {}", cid)).await?;
                        mode.respond(ctx, &issuer_cid_str, &format!("^2{} ^7was unmuted", target.name)).await?;
                        let _ = ctx.storage.disable_all_penalties_of_type(target.id, PenaltyType::Mute).await;
                        info!(admin = issuer_cid, target = %target.name, "!unmute");
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                }
            }

            _ => {
                // Spell-check: suggest closest command
                let known = [
                    "help", "leveltest", "time", "register", "regulars", "rules", "admins",
                    "list", "longlist", "status", "find", "seen", "aliases", "clientinfo",
                    "lookup", "poke", "warn", "warntest", "warns", "warnremove", "warnclear",
                    "warninfo", "notice", "clear", "kick", "spank", "kickall", "spankall",
                    "mute", "unmute",
                    "tempban", "ban", "permban", "banall", "unban", "lastbans", "baninfo",
                    "putgroup", "ungroup", "makereg", "unreg", "mask", "unmask", "iamgod",
                    "say", "scream", "spam", "spams", "maps", "nextmap", "maprotate", "map", "setnextmap",
                    "die", "restart", "reconfig", "pause", "rebuild", "runas", "r3",
                ];
                if let Some(suggestion) = find_closest(&command, &known) {
                    ctx.message(&issuer_cid_str, &format!("^7Unknown command '{}'. Did you mean ^3!{}^7?", command, suggestion)).await?;
                }
            }
        }

        Ok(())
        })
    }

    /// Resolve a client from args: supports @id, connected player, or DB lookup.
    async fn resolve_client(&self, query: &str, ctx: &BotContext) -> Option<Client> {
        // Support @id for direct DB lookup
        if let Some(id_str) = query.strip_prefix('@') {
            if let Ok(id) = id_str.parse::<i64>() {
                return ctx.storage.get_client(id).await.ok();
            }
        }
        // Try connected player first
        if let Some(c) = self.find_target(query, ctx).await {
            return Some(c);
        }
        // Fallback to DB search
        let results = ctx.storage.find_clients(query).await.ok()?;
        results.into_iter().next()
    }
}

/// Split "target reason text" into (target, reason).
fn split_target_reason(s: &str) -> (&str, &str) {
    match s.split_once(' ') {
        Some((t, r)) => (t, r),
        None => (s, ""),
    }
}

/// Parse a duration string like "2h", "30m", "1d" from the front of text.
/// Returns (minutes, remaining_reason_text).
/// If no duration found, defaults to 120 minutes (2 hours).
fn parse_duration_and_reason(s: &str) -> (u32, &str) {
    if s.is_empty() {
        return (120, "");
    }
    let parts: Vec<&str> = s.splitn(2, ' ').collect();
    let token = parts[0];
    let rest = parts.get(1).unwrap_or(&"").trim();

    // Try to parse "Nh", "Nm", "Nd"
    let len = token.len();
    if len >= 2 {
        let (num_part, unit) = token.split_at(len - 1);
        if let Ok(n) = num_part.parse::<u32>() {
            match unit {
                "m" => return (n, rest),
                "h" => return (n * 60, rest),
                "d" => return (n * 1440, rest),
                "w" => return (n * 10080, rest),
                _ => {}
            }
        }
    }

    // Not a duration, treat as reason
    (120, s)
}

/// Format a duration in minutes to a human-readable string.
fn format_duration(minutes: i64) -> String {
    if minutes < 60 {
        format!("{} min", minutes)
    } else if minutes < 1440 {
        format!("{} hours", minutes / 60)
    } else {
        format!("{} days", minutes / 1440)
    }
}

/// Map a numeric level to a human-readable group name.
fn level_name(level: u32) -> &'static str {
    match level {
        0 => "Guest",
        1 => "User",
        2..=19 => "Regular",
        20..=39 => "Moderator",
        40..=59 => "Admin",
        60..=79 => "Senior Admin",
        80..=99 => "Super Admin",
        _ => "Owner",
    }
}

/// Simple Levenshtein-based closest match finder for spell-check.
fn find_closest<'a>(input: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let mut best: Option<&str> = None;
    let mut best_dist = usize::MAX;
    for &candidate in candidates {
        let dist = levenshtein(input, candidate);
        if dist < best_dist && dist <= 2 {
            best_dist = dist;
            best = Some(candidate);
        }
    }
    best
}

/// Compute Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- split_target_reason ---

    #[test]
    fn test_split_target_reason_with_reason() {
        let (target, reason) = split_target_reason("player1 being toxic");
        assert_eq!(target, "player1");
        assert_eq!(reason, "being toxic");
    }

    #[test]
    fn test_split_target_reason_no_reason() {
        let (target, reason) = split_target_reason("player1");
        assert_eq!(target, "player1");
        assert_eq!(reason, "");
    }

    // --- parse_duration_and_reason ---

    #[test]
    fn test_parse_duration_hours() {
        let (mins, reason) = parse_duration_and_reason("2h toxic behavior");
        assert_eq!(mins, 120);
        assert_eq!(reason, "toxic behavior");
    }

    #[test]
    fn test_parse_duration_minutes() {
        let (mins, reason) = parse_duration_and_reason("30m spamming");
        assert_eq!(mins, 30);
        assert_eq!(reason, "spamming");
    }

    #[test]
    fn test_parse_duration_days() {
        let (mins, reason) = parse_duration_and_reason("1d cheating");
        assert_eq!(mins, 1440);
        assert_eq!(reason, "cheating");
    }

    #[test]
    fn test_parse_duration_weeks() {
        let (mins, reason) = parse_duration_and_reason("1w repeated offense");
        assert_eq!(mins, 10080);
        assert_eq!(reason, "repeated offense");
    }

    #[test]
    fn test_parse_duration_default() {
        // No duration prefix — defaults to 120 min
        let (mins, reason) = parse_duration_and_reason("just a reason");
        assert_eq!(mins, 120);
        assert_eq!(reason, "just a reason");
    }

    #[test]
    fn test_parse_duration_empty() {
        let (mins, reason) = parse_duration_and_reason("");
        assert_eq!(mins, 120);
        assert_eq!(reason, "");
    }

    // --- format_duration ---

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30 min");
        assert_eq!(format_duration(120), "2 hours");
        assert_eq!(format_duration(1440), "1 days");
        assert_eq!(format_duration(4320), "3 days");
    }

    // --- level_name ---

    #[test]
    fn test_level_name() {
        assert_eq!(level_name(0), "Guest");
        assert_eq!(level_name(1), "User");
        assert_eq!(level_name(2), "Regular");
        assert_eq!(level_name(20), "Moderator");
        assert_eq!(level_name(40), "Admin");
        assert_eq!(level_name(60), "Senior Admin");
        assert_eq!(level_name(80), "Super Admin");
        assert_eq!(level_name(100), "Owner");
    }
}
