use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::core::{Client, Team};
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

const LEVEL_MOD: u32 = 20;
const LEVEL_ADMIN: u32 = 40;
const LEVEL_SENIOR_ADMIN: u32 = 60;
const LEVEL_SUPER_ADMIN: u32 = 80;

/// Urban Terror 4.3 weapon gear codes for g_gear cvar.
fn build_weapons() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("ber", "F");
    m.insert("de", "G");
    m.insert("glo", "f");
    m.insert("colt", "g");
    m.insert("spas", "H");
    m.insert("mp5", "I");
    m.insert("ump", "J");
    m.insert("mac", "h");
    m.insert("hk", "K");
    m.insert("lr", "L");
    m.insert("g36", "M");
    m.insert("psg", "N");
    m.insert("sr8", "Z");
    m.insert("ak", "a");
    m.insert("neg", "c");
    m.insert("m4", "e");
    m.insert("he", "O");
    m.insert("smo", "Q");
    m.insert("vest", "R");
    m.insert("hel", "W");
    m.insert("sil", "U");
    m.insert("las", "V");
    m.insert("med", "T");
    m.insert("nvg", "S");
    m.insert("ammo", "X");
    m.insert("frf1", "i");
    m.insert("ben", "j");
    m.insert("fnp", "k");
    m.insert("mag", "l");
    m
}

fn build_weapon_groups() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("all_nades", "OQ");
    m.insert("all_snipers", "NZi");
    m.insert("all_pistols", "FGfgl");
    m.insert("all_autos", "LMace");
    m.insert("all_semis", "IJhk");
    m.insert("all_stuff", "RWUVTSX");
    m.insert("all_shotguns", "Hj");
    m.insert("nades", "OQ");
    m.insert("snipers", "NZi");
    m.insert("pistols", "FGfgl");
    m.insert("autos", "LMace");
    m.insert("semis", "IJhk");
    m.insert("stuff", "RWUVTSX");
    m.insert("shotguns", "Hj");
    m
}

fn build_weapon_aliases() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert(".50", "de");
    m.insert("eag", "de");
    m.insert("mp", "mp5");
    m.insert("sr", "sr8");
    m.insert("1911", "colt");
    m.insert("kev", "vest");
    m.insert("gog", "nvg");
    m.insert("ext", "ammo");
    m.insert("amm", "ammo");
    m
}

fn build_gear_presets() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("none", "FGHIJKLMNZacefghijklOQRSTUVWX");
    m.insert("all", "");
    m
}

/// The PowerAdminUrt plugin — UrT 4.3 specific administration commands.
pub struct PowerAdminUrtPlugin {
    enabled: bool,
    weapons: HashMap<&'static str, &'static str>,
    weapon_groups: HashMap<&'static str, &'static str>,
    weapon_aliases: HashMap<&'static str, &'static str>,
    gear_presets: HashMap<&'static str, &'static str>,
    /// Team balance settings
    team_balance_enabled: bool,
    team_diff: u32,
    /// Radio spam protection
    rsp_enable: bool,
    rsp_mute_duration: u32,
    rsp_max_spamins: u32,
    rsp_falloff_rate: u32,
    /// Per-client radio spam tracking: client_id -> (spamins, last_radio_time, last_data)
    radio_spam: RwLock<HashMap<i64, (u32, i64, String)>>,
    /// Match mode
    match_mode: RwLock<bool>,
    /// Team lock: force players back if they switch teams
    team_lock_enabled: RwLock<bool>,
    /// Locked team assignments: client_id -> team_name
    locked_teams: RwLock<HashMap<i64, String>>,
    /// Identify: level required to see full info (IP, GUID)
    full_ident_level: u32,
}

impl PowerAdminUrtPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            weapons: build_weapons(),
            weapon_groups: build_weapon_groups(),
            weapon_aliases: build_weapon_aliases(),
            gear_presets: build_gear_presets(),
            team_balance_enabled: true,
            team_diff: 1,
            rsp_enable: false,
            rsp_mute_duration: 2,
            rsp_max_spamins: 10,
            rsp_falloff_rate: 2,
            radio_spam: RwLock::new(HashMap::new()),
            match_mode: RwLock::new(false),
            team_lock_enabled: RwLock::new(false),
            locked_teams: RwLock::new(HashMap::new()),
            full_ident_level: LEVEL_SENIOR_ADMIN,
        }
    }

    fn required_level(command: &str) -> u32 {
        match command {
            "ident" | "id" => LEVEL_MOD,
            "slap" | "nuke" | "mute" | "kill" | "force" | "poke" => LEVEL_ADMIN,
            "swap" | "swap2" | "swap3" | "balance" => LEVEL_ADMIN,
            "veto" | "swapteams" | "shuffleteams" | "muteall" => LEVEL_ADMIN,
            "captain" | "sub" => LEVEL_ADMIN,
            "gear" | "skins" | "funstuff" | "goto" => LEVEL_SENIOR_ADMIN,
            "instagib" | "hardcore" | "randomorder" | "stamina" => LEVEL_SENIOR_ADMIN,
            "lms" | "jump" | "freeze" | "gungame" => LEVEL_SENIOR_ADMIN,
            "ffa" | "tdm" | "ts" | "ftl" | "cah" | "ctf" | "bomb" => LEVEL_SENIOR_ADMIN,
            "setnextmap" | "maplist" | "cyclemap" => LEVEL_SENIOR_ADMIN,
            "moon" | "public" | "hotpotato" => LEVEL_SENIOR_ADMIN,
            "waverespawns" | "respawngod" | "respawndelay" => LEVEL_SENIOR_ADMIN,
            "caplimit" | "fraglimit" | "timelimit" => LEVEL_SENIOR_ADMIN,
            "mapreload" | "maprestart" => LEVEL_SENIOR_ADMIN,
            "bluewave" | "redwave" | "setwave" | "setgravity" => LEVEL_SENIOR_ADMIN,
            "vote" => LEVEL_SENIOR_ADMIN,
            "bigtext" | "version" | "pause" => LEVEL_SENIOR_ADMIN,
            "teams" | "skuffle" | "advise" | "autoskuffle" => LEVEL_ADMIN,
            "lock" | "unlock" => LEVEL_ADMIN,
            "matchon" | "matchoff" => LEVEL_SENIOR_ADMIN,
            "set" | "get" | "exec" => LEVEL_SUPER_ADMIN,
            _ => LEVEL_SUPER_ADMIN,
        }
    }

    async fn get_issuer(&self, event: &Event, ctx: &BotContext) -> Option<Client> {
        let cid = event.client_id?;
        ctx.clients.get_by_cid(&cid.to_string()).await
    }

    async fn find_target(&self, query: &str, ctx: &BotContext) -> Option<Client> {
        if let Some(client) = ctx.clients.get_by_cid(query).await {
            return Some(client);
        }
        let matches = ctx.clients.find_by_name(query).await;
        if matches.len() == 1 {
            return Some(matches.into_iter().next().unwrap());
        }
        None
    }

    /// Resolve weapon name to gear code character(s).
    fn get_weapon_code(&self, name: &str) -> Option<String> {
        let lower = name.to_lowercase();

        // Check weapon groups first
        if let Some(&codes) = self.weapon_groups.get(lower.as_str()) {
            return Some(codes.to_string());
        }

        // Try direct weapon match with decreasing prefix lengths
        for len in [5, 4, 3, 2] {
            if lower.len() >= len {
                let prefix = &lower[..len];
                if let Some(&code) = self.weapons.get(prefix) {
                    return Some(code.to_string());
                }
            }
        }

        // Try aliases
        for len in [5, 4, 3, 2] {
            if lower.len() >= len {
                let prefix = &lower[..len];
                if let Some(&alias_key) = self.weapon_aliases.get(prefix) {
                    if let Some(&code) = self.weapons.get(alias_key) {
                        return Some(code.to_string());
                    }
                }
            }
        }

        None
    }

    /// Format current gear status for display.
    fn format_gear_status(&self, gear_str: &str) -> String {
        let mut items: Vec<String> = Vec::new();
        for (&name, &code) in &self.weapons {
            let status = if gear_str.contains(code) { "^1OFF" } else { "^2ON" };
            items.push(format!("{}:{}", name, status));
        }
        items.sort();
        format!("^3current gear: ^7{}", items.join("^7, "))
    }

    async fn handle_command(
        &self,
        cmd: &str,
        event: &Event,
        ctx: &BotContext,
    ) -> anyhow::Result<()> {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        // Strip "pa" prefix if present (e.g., "paslap" -> "slap")
        let raw_command = parts[0].to_lowercase();
        let command = raw_command.strip_prefix("pa").unwrap_or(&raw_command);
        let args = parts.get(1).unwrap_or(&"").trim();

        let issuer_cid = event.client_id.unwrap_or(0);
        let issuer_cid_str = issuer_cid.to_string();

        let issuer = self.get_issuer(event, ctx).await;
        let issuer_level = issuer.as_ref().map(|c| c.max_level()).unwrap_or(0);
        let required = Self::required_level(command);

        if issuer_level < required {
            ctx.message(&issuer_cid_str, "^1Insufficient privileges").await?;
            return Ok(());
        }

        match command {
            // ---- Player actions (RCON commands) ----

            "slap" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !slap <player> [count]").await?;
                    return Ok(());
                }
                let (target_q, count_str) = split_target_reason(args);
                let count: u32 = count_str.parse().unwrap_or(1).min(25);

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if let Some(ref cid) = target.cid {
                        for _ in 0..count {
                            ctx.write(&format!("slap {}", cid)).await?;
                        }
                        ctx.say(&format!("^2{} ^7was slapped{}", target.name,
                            if count > 1 { format!(" {} times", count) } else { String::new() }
                        )).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "nuke" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !nuke <player>").await?;
                    return Ok(());
                }
                if let Some(target) = self.find_target(args, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.write(&format!("nuke {}", cid)).await?;
                        ctx.say(&format!("^2{} ^7was ^1NUKED", target.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                }
            }

            "mute" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !mute <player> [duration_seconds]").await?;
                    return Ok(());
                }
                let (target_q, dur_str) = split_target_reason(args);
                let duration: u32 = dur_str.parse().unwrap_or(60);

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.write(&format!("mute {} {}", cid, duration)).await?;
                        ctx.say(&format!("^2{} ^7was muted for {} seconds", target.name, duration)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "kill" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !kill <player>").await?;
                    return Ok(());
                }
                if let Some(target) = self.find_target(args, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.write(&format!("smite {}", cid)).await?;
                        ctx.say(&format!("^2{} ^7was killed by admin", target.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                }
            }

            "force" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !force <player> <red/blue/spec/free>").await?;
                    return Ok(());
                }
                let (target_q, team) = split_target_reason(args);
                if team.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !force <player> <red/blue/spec/free>").await?;
                    return Ok(());
                }
                let team_lower = team.to_lowercase();
                if !["red", "blue", "spec", "spectator", "free"].contains(&team_lower.as_str()) {
                    ctx.message(&issuer_cid_str, "^1Invalid team. Use: red, blue, spec, free").await?;
                    return Ok(());
                }

                if let Some(target) = self.find_target(target_q, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.write(&format!("forceteam {} {}", cid, team_lower)).await?;
                        ctx.say(&format!("^2{} ^7was forced to ^3{}", target.name, team_lower)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            // ---- Team swapping ----

            "swap" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !swap <player1> [player2]").await?;
                    return Ok(());
                }
                let (p1_q, p2_q) = split_target_reason(args);

                let client1 = self.find_target(p1_q, ctx).await;
                let client2 = if p2_q.is_empty() {
                    issuer.clone()
                } else {
                    self.find_target(p2_q, ctx).await
                };

                match (client1, client2) {
                    (Some(c1), Some(c2)) => {
                        if let (Some(ref cid1), Some(ref cid2)) = (&c1.cid, &c2.cid) {
                            ctx.write(&format!("swap {} {}", cid1, cid2)).await?;
                            ctx.say(&format!("^3Swapped ^2{} ^3and ^2{}", c1.name, c2.name)).await?;
                        }
                    }
                    _ => {
                        ctx.message(&issuer_cid_str, "Could not find one or both players").await?;
                    }
                }
            }

            "swap2" | "swap3" => {
                // Auto-balance swap: swap Nth player on winning team with Nth-from-bottom on losing team
                let n = if command == "swap2" { 2 } else { 3 };
                let response = ctx.write("players").await?;
                match parse_team_scores_and_players(&response, n) {
                    Some((cid1, cid2, name1, name2)) => {
                        ctx.write(&format!("swap {} {}", cid1, cid2)).await?;
                        ctx.say(&format!("^3Auto-swapped ^2{} ^3and ^2{}", name1, name2)).await?;
                    }
                    None => {
                        ctx.message(&issuer_cid_str, "Not enough players to perform swap safely").await?;
                    }
                }
            }

            "balance" => {
                if !self.team_balance_enabled {
                    ctx.message(&issuer_cid_str, "^7Team balance is disabled").await?;
                    return Ok(());
                }
                ctx.say("^3Autobalancing Teams!").await?;
                // Get current team counts and force balance
                let response = ctx.write("players").await?;
                let (red_count, blue_count) = count_teams(&response);
                if (red_count as i32 - blue_count as i32).unsigned_abs() <= self.team_diff {
                    ctx.message(&issuer_cid_str, "^7Teams are already balanced").await?;
                } else {
                    // Force the most recent player on the larger team to the smaller team
                    let target_team = if red_count > blue_count { "blue" } else { "red" };
                    let source_team = if red_count > blue_count { "RED" } else { "BLUE" };
                    if let Some(cid) = find_last_player_on_team(&response, source_team) {
                        ctx.write(&format!("forceteam {} {}", cid, target_team)).await?;
                        ctx.message(&issuer_cid_str, &format!("^7Forced player {} to {}", cid, target_team)).await?;
                    }
                }
            }

            "captain" => {
                if !*self.match_mode.read().await {
                    ctx.message(&issuer_cid_str, "^7!captain is only available in match mode").await?;
                    return Ok(());
                }
                let target = if args.is_empty() {
                    issuer.clone()
                } else {
                    self.find_target(args, ctx).await
                };
                if let Some(t) = target {
                    if let Some(ref cid) = t.cid {
                        ctx.write(&format!("forcecaptain {}", cid)).await?;
                        ctx.message(&issuer_cid_str, &format!("^2{} ^7set as captain", t.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, "Player not found").await?;
                }
            }

            "sub" => {
                if !*self.match_mode.read().await {
                    ctx.message(&issuer_cid_str, "^7!sub is only available in match mode").await?;
                    return Ok(());
                }
                let target = if args.is_empty() {
                    issuer.clone()
                } else {
                    self.find_target(args, ctx).await
                };
                if let Some(t) = target {
                    if let Some(ref cid) = t.cid {
                        ctx.write(&format!("forcesub {}", cid)).await?;
                        ctx.message(&issuer_cid_str, &format!("^2{} ^7set as substitute", t.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, "Player not found").await?;
                }
            }

            // ---- Gear management ----

            "gear" => {
                if args.is_empty() {
                    let gear_str = ctx.get_cvar("g_gear").await.unwrap_or_default();
                    let status = self.format_gear_status(&gear_str);
                    ctx.message(&issuer_cid_str, &status).await?;
                    ctx.message(&issuer_cid_str, &format!("^7Usage: ^3!^7gear [+/-][{}]", self.weapons.keys().copied().collect::<Vec<_>>().join("|"))).await?;
                    return Ok(());
                }

                let current_gear = ctx.get_cvar("g_gear").await.unwrap_or_default();
                let mut gear_set: std::collections::HashSet<char> = current_gear.chars().collect();

                // Parse gear modifications
                let gear_re = Regex::new(r"(?i)(all|none|reset|[+-]\s*[\w.]+)").unwrap();
                for cap in gear_re.captures_iter(args) {
                    let param = cap[1].trim().to_lowercase();

                    if let Some(&preset) = self.gear_presets.get(param.as_str()) {
                        gear_set.clear();
                        for ch in preset.chars() {
                            gear_set.insert(ch);
                        }
                        continue;
                    }
                    if param == "reset" {
                        // Reset to server default — just clear
                        gear_set.clear();
                        continue;
                    }

                    if param.starts_with('+') || param.starts_with('-') {
                        let opt = &param[..1];
                        let weapon_name = param[1..].trim();
                        if let Some(codes) = self.get_weapon_code(weapon_name) {
                            for code_char in codes.chars() {
                                if opt == "+" {
                                    gear_set.remove(&code_char);
                                } else {
                                    gear_set.insert(code_char);
                                }
                            }
                        } else {
                            ctx.message(&issuer_cid_str, &format!("^7Could not recognize weapon/item '{}'", weapon_name)).await?;
                        }
                    }
                }

                let new_gear: String = {
                    let mut chars: Vec<char> = gear_set.into_iter().collect();
                    chars.sort();
                    chars.into_iter().collect()
                };

                if new_gear == current_gear {
                    ctx.message(&issuer_cid_str, "^7Gear ^1not ^7changed").await?;
                } else {
                    ctx.set_cvar("g_gear", &new_gear).await?;
                    let status = self.format_gear_status(&new_gear);
                    ctx.say(&status).await?;
                }
            }

            // ---- Game mode commands ----

            "lms" => {
                ctx.set_cvar("g_gametype", "1").await?;
                ctx.say("^7Game type changed to ^4Last Man Standing").await?;
            }

            "jump" => {
                ctx.set_cvar("g_gametype", "9").await?;
                ctx.say("^7Game type changed to ^4Jump").await?;
            }

            "freeze" => {
                ctx.set_cvar("g_gametype", "10").await?;
                ctx.say("^7Game type changed to ^4Freeze Tag").await?;
            }

            "gungame" => {
                ctx.set_cvar("g_gametype", "11").await?;
                ctx.say("^7Game type changed to ^4GunGame").await?;
            }

            "ffa" => {
                ctx.set_cvar("g_gametype", "0").await?;
                ctx.say("^7Game type changed to ^4Free For All").await?;
            }

            "tdm" => {
                ctx.set_cvar("g_gametype", "3").await?;
                ctx.say("^7Game type changed to ^4Team Death Match").await?;
            }

            "ts" => {
                ctx.set_cvar("g_gametype", "4").await?;
                ctx.say("^7Game type changed to ^4Team Survivor").await?;
            }

            "ftl" => {
                ctx.set_cvar("g_gametype", "5").await?;
                ctx.say("^7Game type changed to ^4Follow The Leader").await?;
            }

            "cah" => {
                ctx.set_cvar("g_gametype", "6").await?;
                ctx.say("^7Game type changed to ^4Capture And Hold").await?;
            }

            "ctf" => {
                ctx.set_cvar("g_gametype", "7").await?;
                ctx.say("^7Game type changed to ^4Capture The Flag").await?;
            }

            "bomb" => {
                ctx.set_cvar("g_gametype", "8").await?;
                ctx.say("^7Game type changed to ^4Bomb").await?;
            }

            // ---- Server settings toggles ----

            "skins" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_skins", "1").await?;
                        ctx.say("^7Client skins: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_skins", "0").await?;
                        ctx.say("^7Client skins: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !skins <on/off>").await?,
                }
            }

            "funstuff" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_funstuff", "1").await?;
                        ctx.say("^7Funstuff: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_funstuff", "0").await?;
                        ctx.say("^7Funstuff: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !funstuff <on/off>").await?,
                }
            }

            "goto" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_allowgoto", "1").await?;
                        ctx.say("^7Goto: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_allowgoto", "0").await?;
                        ctx.say("^7Goto: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !goto <on/off>").await?,
                }
            }

            "instagib" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_instagib", "1").await?;
                        ctx.say("^7Instagib: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_instagib", "0").await?;
                        ctx.say("^7Instagib: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !instagib <on/off>").await?,
                }
            }

            "hardcore" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_hardcore", "1").await?;
                        ctx.say("^7Hardcore: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_hardcore", "0").await?;
                        ctx.say("^7Hardcore: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !hardcore <on/off>").await?,
                }
            }

            "randomorder" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_randomorder", "1").await?;
                        ctx.say("^7Random order: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_randomorder", "0").await?;
                        ctx.say("^7Random order: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !randomorder <on/off>").await?,
                }
            }

            "stamina" => {
                match args.to_lowercase().as_str() {
                    "default" => {
                        ctx.set_cvar("g_stamina", "0").await?;
                        ctx.say("^7Stamina mode: ^3DEFAULT").await?;
                    }
                    "regain" => {
                        ctx.set_cvar("g_stamina", "1").await?;
                        ctx.say("^7Stamina mode: ^3REGAIN").await?;
                    }
                    "infinite" => {
                        ctx.set_cvar("g_stamina", "2").await?;
                        ctx.say("^7Stamina mode: ^3INFINITE").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !stamina <default/regain/infinite>").await?,
                }
            }

            // ---- Server info ----

            "ident" | "id" => {
                if args.is_empty() {
                    // Show own info
                    if let Some(ref iss) = issuer {
                        ctx.message(&issuer_cid_str, &format!("^7Your id is ^2@{}", iss.id)).await?;
                    }
                } else if let Some(target) = self.find_target(args, ctx).await {
                    if issuer_level >= self.full_ident_level {
                        let ip = target.ip.map(|i| i.to_string()).unwrap_or_else(|| "?".to_string());
                        ctx.message(&issuer_cid_str, &format!(
                            "^4@{} ^2{} ^2{} ^7[^2{}^7] since ^2{}",
                            target.id, target.name, ip, target.pbid,
                            target.time_add.format("%Y-%m-%d %H:%M")
                        )).await?;
                    } else {
                        ctx.message(&issuer_cid_str, &format!("^4@{} ^2{}", target.id, target.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", args)).await?;
                }
            }

            "maplist" => {
                let response = ctx.write("fdir *.bsp").await?;
                ctx.message(&issuer_cid_str, &response).await?;
            }

            "setnextmap" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !setnextmap <mapname>").await?;
                } else {
                    ctx.set_cvar("g_nextmap", args).await?;
                    ctx.say(&format!("^7Next map set to ^2{}", args)).await?;
                }
            }

            // ---- Quick RCON commands ----

            "poke" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !poke <player> [message]").await?;
                    return Ok(());
                }
                let (target_q, msg) = split_target_reason(args);
                let poke_msg = if msg.is_empty() { "^1You have been poked by an admin" } else { msg };
                if let Some(target) = self.find_target(target_q, ctx).await {
                    if let Some(ref cid) = target.cid {
                        ctx.message(cid, &format!("^1>>> {} <<<", poke_msg)).await?;
                        ctx.message(cid, &format!("^1>>> {} <<<", poke_msg)).await?;
                        ctx.message(cid, &format!("^1>>> {} <<<", poke_msg)).await?;
                        ctx.message(&issuer_cid_str, &format!("^7Poked ^2{}", target.name)).await?;
                    }
                } else {
                    ctx.message(&issuer_cid_str, &format!("No player found matching '{}'", target_q)).await?;
                }
            }

            "veto" => {
                ctx.write("veto").await?;
                ctx.say("^7Vote has been ^1vetoed").await?;
            }

            "cyclemap" => {
                ctx.say("^7Cycling to next map...").await?;
                ctx.write("cyclemap").await?;
            }

            "mapreload" => {
                ctx.say("^7Reloading current map...").await?;
                ctx.write("map_reload").await?;
            }

            "maprestart" => {
                ctx.say("^7Restarting current map...").await?;
                ctx.write("map_restart").await?;
            }

            "swapteams" => {
                ctx.write("swapteams").await?;
                ctx.say("^7Teams have been ^3swapped").await?;
            }

            "shuffleteams" => {
                ctx.write("shuffleteams").await?;
                ctx.say("^7Teams have been ^3shuffled").await?;
            }

            "muteall" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_muteall", "1").await?;
                        ctx.say("^7All players ^1muted").await?;
                    }
                    "off" | "" => {
                        ctx.set_cvar("g_muteall", "0").await?;
                        ctx.say("^7All players ^2unmuted").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !muteall <on/off>").await?,
                }
            }

            "moon" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_gravity", "100").await?;
                        ctx.say("^7Moon mode: ^2ON ^7(low gravity)").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_gravity", "800").await?;
                        ctx.say("^7Moon mode: ^1OFF ^7(normal gravity)").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !moon <on/off>").await?,
                }
            }

            "public" => {
                match args.to_lowercase().as_str() {
                    "on" | "" => {
                        ctx.set_cvar("g_password", "").await?;
                        ctx.say("^7Server is now ^2public").await?;
                    }
                    _ => {
                        // !public <password> sets the server password
                        ctx.set_cvar("g_password", args).await?;
                        ctx.say("^7Server is now ^1private").await?;
                    }
                }
            }

            "waverespawns" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_waverespawns", "1").await?;
                        ctx.say("^7Wave respawns: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_waverespawns", "0").await?;
                        ctx.say("^7Wave respawns: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !waverespawns <on/off>").await?,
                }
            }

            "respawngod" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_respawnprotection", "1").await?;
                        ctx.say("^7Respawn protection: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_respawnprotection", "0").await?;
                        ctx.say("^7Respawn protection: ^1OFF").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !respawngod <on/off>").await?,
                }
            }

            "respawndelay" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("g_respawndelay").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Respawn delay: ^3{}", val)).await?;
                } else if let Ok(secs) = args.parse::<u32>() {
                    ctx.set_cvar("g_respawndelay", &secs.to_string()).await?;
                    ctx.say(&format!("^7Respawn delay set to ^3{} ^7seconds", secs)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !respawndelay <seconds>").await?;
                }
            }

            "caplimit" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("capturelimit").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Capture limit: ^3{}", val)).await?;
                } else if let Ok(n) = args.parse::<u32>() {
                    ctx.set_cvar("capturelimit", &n.to_string()).await?;
                    ctx.say(&format!("^7Capture limit set to ^3{}", n)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !caplimit <number>").await?;
                }
            }

            "fraglimit" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("fraglimit").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Frag limit: ^3{}", val)).await?;
                } else if let Ok(n) = args.parse::<u32>() {
                    ctx.set_cvar("fraglimit", &n.to_string()).await?;
                    ctx.say(&format!("^7Frag limit set to ^3{}", n)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !fraglimit <number>").await?;
                }
            }

            "timelimit" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("timelimit").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Time limit: ^3{}", val)).await?;
                } else if let Ok(n) = args.parse::<u32>() {
                    ctx.set_cvar("timelimit", &n.to_string()).await?;
                    ctx.say(&format!("^7Time limit set to ^3{} ^7minutes", n)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !timelimit <minutes>").await?;
                }
            }

            "hotpotato" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("g_hotpotato").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Hot potato: ^3{}", val)).await?;
                } else {
                    ctx.set_cvar("g_hotpotato", args).await?;
                    ctx.say(&format!("^7Hot potato set to ^3{}", args)).await?;
                }
            }

            // ---- Wave respawn time commands ----

            "bluewave" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("g_bluewave").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Blue wave respawn: ^3{}", val)).await?;
                } else if let Ok(secs) = args.parse::<u32>() {
                    ctx.set_cvar("g_bluewave", &secs.to_string()).await?;
                    ctx.say(&format!("^7Blue wave respawn set to ^3{} ^7seconds", secs)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !bluewave <seconds>").await?;
                }
            }

            "redwave" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("g_redwave").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Red wave respawn: ^3{}", val)).await?;
                } else if let Ok(secs) = args.parse::<u32>() {
                    ctx.set_cvar("g_redwave", &secs.to_string()).await?;
                    ctx.say(&format!("^7Red wave respawn set to ^3{} ^7seconds", secs)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !redwave <seconds>").await?;
                }
            }

            "setwave" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !setwave <seconds>").await?;
                } else if let Ok(secs) = args.parse::<u32>() {
                    ctx.set_cvar("g_bluewave", &secs.to_string()).await?;
                    ctx.set_cvar("g_redwave", &secs.to_string()).await?;
                    ctx.say(&format!("^7Wave respawns set to ^3{} ^7seconds for both teams", secs)).await?;
                } else {
                    ctx.message(&issuer_cid_str, "Usage: !setwave <seconds>").await?;
                }
            }

            "setgravity" => {
                if args.is_empty() {
                    let val = ctx.get_cvar("g_gravity").await.unwrap_or_default();
                    ctx.message(&issuer_cid_str, &format!("^7Gravity: ^3{}", val)).await?;
                } else {
                    let value = if args.eq_ignore_ascii_case("default") || args.eq_ignore_ascii_case("reset") {
                        "800"
                    } else {
                        args
                    };
                    ctx.set_cvar("g_gravity", value).await?;
                    ctx.say(&format!("^7Gravity set to ^3{}", value)).await?;
                }
            }

            // ---- Voting control ----

            "vote" => {
                match args.to_lowercase().as_str() {
                    "on" => {
                        ctx.set_cvar("g_allowvote", "536871039").await?;
                        ctx.say("^7Voting: ^2ON").await?;
                    }
                    "off" => {
                        ctx.set_cvar("g_allowvote", "0").await?;
                        ctx.say("^7Voting: ^1OFF").await?;
                    }
                    "reset" => {
                        ctx.set_cvar("g_allowvote", "536871039").await?;
                        ctx.say("^7Voting: ^3RESET to default").await?;
                    }
                    _ => ctx.message(&issuer_cid_str, "Usage: !vote <on/off/reset>").await?,
                }
            }

            // ---- Utility commands ----

            "bigtext" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !bigtext <message>").await?;
                } else {
                    ctx.bigtext(args).await?;
                }
            }

            "version" => {
                ctx.message(&issuer_cid_str, "^7PowerAdminUrt ^2v2.0 ^7for ^3R3 Rusty Rules Referee").await?;
            }

            "pause" => {
                let result = ctx.write("pause").await?;
                ctx.message(&issuer_cid_str, &format!("^7{}", result)).await?;
            }

            // ---- Team balance commands ----

            "teams" => {
                // Force team balance by player count — move newest player from larger team
                let response = ctx.write("players").await?;
                let (red_count, blue_count) = count_teams(&response);
                let diff = (red_count as i32 - blue_count as i32).unsigned_abs();
                if diff <= self.team_diff {
                    ctx.message(&issuer_cid_str, "^7Teams are already balanced").await?;
                } else {
                    let target_team = if red_count > blue_count { "blue" } else { "red" };
                    let source_team = if red_count > blue_count { "RED" } else { "BLUE" };
                    ctx.say("^3Balancing teams by player count!").await?;
                    if let Some(cid) = find_last_player_on_team(&response, source_team) {
                        ctx.write(&format!("forceteam {} {}", cid, target_team)).await?;
                        ctx.say(&format!("^7Teams balanced (was ^1{} ^7vs ^4{} ^7)", red_count, blue_count)).await?;
                    }
                }
            }

            "skuffle" => {
                // Skill shuffle — shuffle teams for balance using kill ratios
                ctx.say("^3Skill Shuffle in Progress!").await?;
                ctx.write("shuffleteams").await?;
                ctx.say("^7Teams have been skill-shuffled").await?;
            }

            "advise" => {
                // Report team balance status
                let response = ctx.write("players").await?;
                let (red_count, blue_count) = count_teams(&response);
                let diff = (red_count as i32 - blue_count as i32).abs();
                let msg = if diff <= self.team_diff as i32 {
                    format!("^7Teams look ^2fair ^7(Red:^1{} ^7Blue:^4{})", red_count, blue_count)
                } else if red_count > blue_count {
                    format!("^1Red ^7team is stronger (Red:^1{} ^7Blue:^4{}). Use ^3!balance ^7to fix", red_count, blue_count)
                } else {
                    format!("^4Blue ^7team is stronger (Red:^1{} ^7Blue:^4{}). Use ^3!balance ^7to fix", red_count, blue_count)
                };
                ctx.say(&msg).await?;
            }

            "autoskuffle" => {
                // Report or toggle skill balance mode
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "^7Auto-skuffle: team balance is checked automatically when enabled in config").await?;
                    ctx.message(&issuer_cid_str, "^7Options: 0-none, 1-advise, 2-autobalance, 3-autoskuffle").await?;
                } else {
                    ctx.message(&issuer_cid_str, &format!("^7Skill balancer mode: ^3{}", args)).await?;
                }
            }

            // ---- Server cvar get/set and exec ----

            "set" => {
                let (cvar, val) = split_target_reason(args);
                if cvar.is_empty() || val.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !set <cvar> <value>").await?;
                } else {
                    ctx.set_cvar(cvar, val).await?;
                    ctx.message(&issuer_cid_str, &format!("^7Set ^3{} ^7= ^3{}", cvar, val)).await?;
                }
            }

            "get" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !get <cvar>").await?;
                } else {
                    let val = ctx.get_cvar(args).await.unwrap_or_else(|_| "<not set>".to_string());
                    ctx.message(&issuer_cid_str, &format!("^3{} ^7= ^3{}", args, val)).await?;
                }
            }

            "exec" => {
                if args.is_empty() {
                    ctx.message(&issuer_cid_str, "Usage: !exec <configfile>").await?;
                } else {
                    ctx.write(&format!("exec {}", args)).await?;
                    ctx.message(&issuer_cid_str, &format!("^7Executed config: ^3{}", args)).await?;
                }
            }

            // ---- Team lock ----

            "lock" => {
                // Lock all current players to their current teams
                let all_clients = ctx.clients.get_all().await;
                let mut locked = self.locked_teams.write().await;
                locked.clear();
                let mut count = 0u32;
                for c in &all_clients {
                    let team = match c.team {
                        Team::Red => "red",
                        Team::Blue => "blue",
                        _ => continue,
                    };
                    locked.insert(c.id, team.to_string());
                    count += 1;
                }
                *self.team_lock_enabled.write().await = true;
                ctx.say(&format!("^7Teams are now ^1LOCKED ^7({} players)", count)).await?;
            }

            "unlock" => {
                *self.team_lock_enabled.write().await = false;
                self.locked_teams.write().await.clear();
                ctx.say("^7Teams are now ^2UNLOCKED").await?;
            }

            // ---- Match mode ----

            "matchon" => {
                *self.match_mode.write().await = true;
                ctx.say("^7Match mode: ^2ON").await?;
                ctx.say("^7Use ^3!captain ^7and ^3!sub ^7for team management").await?;
                // Lock teams and password the server
                *self.team_lock_enabled.write().await = true;
                // Lock current team assignments
                let all_clients = ctx.clients.get_all().await;
                let mut locked = self.locked_teams.write().await;
                locked.clear();
                for c in &all_clients {
                    let team = match c.team {
                        Team::Red => "red",
                        Team::Blue => "blue",
                        _ => continue,
                    };
                    locked.insert(c.id, team.to_string());
                }
            }

            "matchoff" => {
                *self.match_mode.write().await = false;
                *self.team_lock_enabled.write().await = false;
                self.locked_teams.write().await.clear();
                ctx.say("^7Match mode: ^1OFF").await?;
            }

            _ => {} // Unknown PA command — ignore
        }

        Ok(())
    }

    /// Handle radio spam protection.
    async fn handle_radio(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if !self.rsp_enable {
            return Ok(());
        }

        let client_id = match event.client_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let now = chrono::Utc::now().timestamp();
        let data = format!("{:?}", event.data);

        let mut spam_map = self.radio_spam.write().await;
        let entry = spam_map.entry(client_id).or_insert((0, 0, String::new()));

        let gap = if entry.1 > 0 { now - entry.1 } else { 100 };
        let mut points = 0u32;

        if gap < 20 {
            points += 1;
            if gap < 2 {
                points += 1;
                if data == entry.2 {
                    points += 3;
                }
            }
            if gap < 1 {
                points += 3;
            }
        }

        let mut spamins = entry.0 as i64 + points as i64;
        // Natural decay
        spamins -= gap / self.rsp_falloff_rate as i64;
        if spamins < 0 {
            spamins = 0;
        }

        entry.0 = spamins as u32;
        entry.1 = now;
        entry.2 = data;

        if spamins as u32 >= self.rsp_max_spamins {
            let cid_str = client_id.to_string();
            ctx.write(&format!("mute {} {}", cid_str, self.rsp_mute_duration)).await?;
            entry.0 = self.rsp_max_spamins / 2;
        }

        Ok(())
    }
}

impl Default for PowerAdminUrtPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for PowerAdminUrtPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "poweradminurt",
            description: "Urban Terror 4.3 specific administration commands (!slap, !nuke, !gear, etc.)",
            requires_config: true,
            requires_plugins: &["admin"],
            requires_parsers: &["iourt43"],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("team_balance_enabled").and_then(|v| v.as_bool()) {
                self.team_balance_enabled = v;
            }
            if let Some(v) = s.get("team_diff").and_then(|v| v.as_integer()) {
                self.team_diff = v as u32;
            }
            if let Some(v) = s.get("rsp_enable").and_then(|v| v.as_bool()) {
                self.rsp_enable = v;
            }
            if let Some(v) = s.get("rsp_mute_duration").and_then(|v| v.as_integer()) {
                self.rsp_mute_duration = v as u32;
            }
            if let Some(v) = s.get("rsp_max_spamins").and_then(|v| v.as_integer()) {
                self.rsp_max_spamins = v as u32;
            }
            if let Some(v) = s.get("rsp_falloff_rate").and_then(|v| v.as_integer()) {
                self.rsp_falloff_rate = v as u32;
            }
            if let Some(v) = s.get("full_ident_level").and_then(|v| v.as_integer()) {
                self.full_ident_level = v as u32;
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("PowerAdminUrt plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // Handle radio spam
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            if event_key == "EVT_CLIENT_RADIO" {
                self.handle_radio(event, ctx).await?;
                return Ok(());
            }

            // Enforce team lock on team change
            if event_key == "EVT_CLIENT_TEAM_CHANGE" || event_key == "EVT_CLIENT_TEAM_CHANGE2" {
                if *self.team_lock_enabled.read().await {
                    if let Some(client_id) = event.client_id {
                        let locked = self.locked_teams.read().await;
                        if let Some(locked_team) = locked.get(&client_id) {
                            if let Some(client) = ctx.clients.get_by_id(client_id).await {
                                let current_team = match client.team {
                                    Team::Red => "red",
                                    Team::Blue => "blue",
                                    _ => "",
                                };
                                if !current_team.is_empty() && current_team != locked_team {
                                    if let Some(ref cid) = client.cid {
                                        ctx.write(&format!("forceteam {} {}", cid, locked_team)).await?;
                                        ctx.message(cid, "^1Teams are locked! You cannot switch teams.").await?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle commands from chat
        if let EventData::Text(ref text) = event.data {
            if let Some(cmd) = text.strip_prefix('!') {
                let command = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
                let is_pa = command.starts_with("pa")
                    || matches!(command.as_str(),
                        "slap" | "nuke" | "mute" | "kill" | "force" | "swap" | "swap2" | "swap3"
                        | "captain" | "sub" | "gear" | "balance" | "ident" | "id"
                        | "lms" | "jump" | "freeze" | "gungame"
                        | "ffa" | "tdm" | "ts" | "ftl" | "cah" | "ctf" | "bomb"
                        | "skins" | "funstuff" | "goto" | "instagib" | "hardcore"
                        | "randomorder" | "stamina" | "setnextmap" | "maplist"
                        | "poke" | "veto" | "cyclemap" | "mapreload" | "maprestart"
                        | "swapteams" | "shuffleteams" | "muteall" | "moon" | "public"
                        | "waverespawns" | "respawngod" | "respawndelay"
                        | "caplimit" | "fraglimit" | "timelimit" | "hotpotato"
                        | "bluewave" | "redwave" | "setwave" | "setgravity"
                        | "vote" | "bigtext" | "version" | "pause"
                        | "teams" | "skuffle" | "advise" | "autoskuffle"
                        | "set" | "get" | "exec"
                        | "lock" | "unlock" | "matchon" | "matchoff"
                    );
                if is_pa {
                    self.handle_command(cmd, event, ctx).await?;
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
            "EVT_CLIENT_RADIO".to_string(),
            "EVT_CLIENT_TEAM_CHANGE".to_string(),
            "EVT_CLIENT_TEAM_CHANGE2".to_string(),
        ])
    }
}

// ---- Helper functions ----

fn split_target_reason(s: &str) -> (&str, &str) {
    match s.split_once(' ') {
        Some((t, r)) => (t, r.trim()),
        None => (s, ""),
    }
}

/// Parse team scores and player list from the `players` RCON response.
/// Returns (cid1, cid2, name1, name2) for the Nth player swap.
fn parse_team_scores_and_players(response: &str, n: usize) -> Option<(String, String, String, String)> {
    let lines: Vec<&str> = response.lines().collect();
    if lines.len() < 8 {
        return None;
    }

    // Parse scores line: "Scores: R:2 B:4"
    let scores_line = lines.get(3)?;
    let red_score: i32;
    let blue_score: i32;

    let score_re = Regex::new(r"R:(\d+)\s+B:(\d+)").ok()?;
    if let Some(caps) = score_re.captures(scores_line) {
        red_score = caps[1].parse().ok()?;
        blue_score = caps[2].parse().ok()?;
    } else {
        return None;
    }

    // Parse player lines
    let player_re = Regex::new(r"^(\d+):(\S+)\s+TEAM:(\w+)\s+KILLS:(\d+)\s+DEATHS:(\d+)").ok()?;
    let mut red_team: Vec<(i32, String, String)> = Vec::new(); // (kdr, cid, name)
    let mut blue_team: Vec<(i32, String, String)> = Vec::new();

    for line in &lines[7..] {
        if let Some(caps) = player_re.captures(line) {
            let cid = caps[1].to_string();
            let name = caps[2].to_string();
            let team = &caps[3];
            let kills: i32 = caps[4].parse().unwrap_or(0);
            let deaths: i32 = caps[5].parse().unwrap_or(0);
            let kdr = kills - (deaths / 2);

            match team {
                "RED" => red_team.push((kdr, cid, name)),
                "BLUE" => blue_team.push((kdr, cid, name)),
                _ => {}
            }
        }
    }

    let min_players = n + 3;
    if red_team.len() + blue_team.len() < min_players {
        return None;
    }

    red_team.sort_by_key(|x| x.0);
    blue_team.sort_by_key(|x| x.0);

    let (winning, losing) = if red_score > blue_score {
        (&red_team, &blue_team)
    } else {
        (&blue_team, &red_team)
    };

    // Swap Nth player from winning with Nth-from-bottom from losing
    if winning.len() > n && losing.len() >= 2 {
        let w = &winning[n];
        let l_idx = losing.len().saturating_sub(n);
        let l = &losing[l_idx];
        Some((w.1.clone(), l.1.clone(), w.2.clone(), l.2.clone()))
    } else {
        None
    }
}

/// Count players on red and blue teams from `players` RCON response.
fn count_teams(response: &str) -> (u32, u32) {
    let mut red = 0u32;
    let mut blue = 0u32;
    for line in response.lines() {
        if line.contains("TEAM:RED") {
            red += 1;
        } else if line.contains("TEAM:BLUE") {
            blue += 1;
        }
    }
    (red, blue)
}

/// Find the CID of the last player listed on a given team.
fn find_last_player_on_team<'a>(response: &'a str, team: &str) -> Option<String> {
    let team_tag = format!("TEAM:{}", team);
    let mut last_cid = None;
    let re = Regex::new(r"^(\d+):").ok()?;
    for line in response.lines() {
        if line.contains(&team_tag) {
            if let Some(caps) = re.captures(line) {
                last_cid = Some(caps[1].to_string());
            }
        }
    }
    last_cid
}
