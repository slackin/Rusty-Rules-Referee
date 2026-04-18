use async_trait::async_trait;
use regex::Regex;
use std::sync::Arc;
use tracing::{debug, info};

use crate::events::{Event, EventData, EventRegistry};
use crate::parsers::{GameCommands, GameParser, LogLine, ParsedAction};
use crate::rcon::RconClient;

/// Urban Terror 4.3 weapon/means-of-death IDs.
/// Maps numeric MOD IDs from kill lines to human-readable names.
fn weapon_name(id: u32) -> &'static str {
    match id {
        1 => "UT_MOD_KNIFE",
        2 => "UT_MOD_BERETTA",
        3 => "UT_MOD_DEAGLE",
        4 => "UT_MOD_SPAS",
        5 => "UT_MOD_MP5K",
        6 => "UT_MOD_UMP45",
        8 => "UT_MOD_LR300",
        9 => "UT_MOD_G36",
        10 => "UT_MOD_PSG1",
        14 => "UT_MOD_SR8",
        15 => "UT_MOD_AK103",
        17 => "UT_MOD_NEGEV",
        19 => "UT_MOD_M4",
        20 => "UT_MOD_GLOCK",
        21 => "UT_MOD_COLT1911",
        22 => "UT_MOD_MAC11",
        23 => "UT_MOD_FRF1",
        24 => "UT_MOD_BENELLI",
        25 => "UT_MOD_P90",
        26 => "UT_MOD_MAGNUM",
        28 => "UT_MOD_TOD50",
        30 => "UT_MOD_FLAG",
        31 => "UT_MOD_KNIFE_THROWN",
        32 => "UT_MOD_HK69",
        34 => "UT_MOD_BLED",
        35 => "UT_MOD_KICKED",
        36 => "UT_MOD_HEGRENADE",
        37 => "UT_MOD_SMKGRENADE",
        38 => "UT_MOD_SR8",
        39 => "UT_MOD_SPLODED",
        40 => "UT_MOD_SLAPPED",
        41 => "UT_MOD_BOMBED",
        42 => "UT_MOD_NUKED",
        43 => "UT_MOD_NEGEV",
        _ => "UT_MOD_UNKNOWN",
    }
}

/// Urban Terror 4.3 hit location IDs.
fn hit_location_name(id: u32) -> &'static str {
    match id {
        0 => "HEAD",
        1 => "HELMET",
        2 => "TORSO",
        3 => "VEST",
        4 => "LEFT_ARM",
        5 => "RIGHT_ARM",
        6 => "GROIN",
        7 => "BUTT",
        8 => "LEFT_UPPER_LEG",
        9 => "RIGHT_UPPER_LEG",
        10 => "LEFT_LOWER_LEG",
        11 => "RIGHT_LOWER_LEG",
        12 => "LEFT_FOOT",
        13 => "RIGHT_FOOT",
        _ => "UNKNOWN",
    }
}

/// Player info returned from RCON `dumpuser` command.
#[derive(Debug, Clone, Default)]
pub struct PlayerInfo {
    pub cid: String,
    pub name: String,
    pub guid: String,
    pub ip: String,
    pub team: String,
    pub auth: String,
}

/// Player info returned from RCON `status` command.
#[derive(Debug, Clone)]
pub struct StatusPlayer {
    pub slot: String,
    pub score: String,
    pub ping: String,
    pub name: String,
    pub address: String,
}

/// Urban Terror 4.3 parser.
///
/// Parses UrT 4.3 game server log lines into B3 events and sends
/// RCON commands using the Quake 3 UDP protocol.
pub struct UrbanTerrorParser {
    pub commands: GameCommands,
    pub rcon: Arc<RconClient>,
    pub event_registry: Arc<EventRegistry>,

    // Compiled regexes for log line parsing
    re_timestamp: Regex,
    re_kill: Regex,
    re_hit: Regex,
    re_say: Regex,
    re_sayteam: Regex,
    re_client_connect: Regex,
    re_client_disconnect: Regex,
    re_client_begin: Regex,
    re_client_userinfo: Regex,
    re_client_userinfo_changed: Regex,
    re_init_game: Regex,
    re_shutdown_game: Regex,
    re_warmup: Regex,
    re_init_round: Regex,
    re_flag: Regex,
    re_bomb: Regex,
    re_survivor_winner: Regex,
    re_account_validated: Regex,
    re_account_rejected: Regex,
    re_item: Regex,
}

impl UrbanTerrorParser {
    pub fn new(rcon: Arc<RconClient>, event_registry: Arc<EventRegistry>) -> Self {
        Self {
            commands: GameCommands::default(),
            rcon,
            event_registry,
            re_timestamp: Regex::new(r"^\s*(?:\d+:\d+)\s+").unwrap(),
            // Kill: <attacker_id> <victim_id> <weapon_id>: <attacker_name> killed <victim_name> by <weapon>
            re_kill: Regex::new(
                r"^Kill:\s+(?P<acid>\d+)\s+(?P<cid>\d+)\s+(?P<weapid>\d+):\s+(?P<aname>.+)\s+killed\s+(?P<name>.+)\s+by\s+(?P<weapon>\S+)$"
            ).unwrap(),
            // Hit: <target_id> <attacker_id> <location> <damage>: <attacker_name> hit <target_name> in the <location_name>
            re_hit: Regex::new(
                r"^Hit:\s+(?P<cid>\d+)\s+(?P<acid>\d+)\s+(?P<hitloc>\d+)\s+(?P<dmg>\d+):\s+(?P<aname>.+)\s+hit\s+(?P<name>.+)\s+in the\s+(?P<hitloc_name>.+)$"
            ).unwrap(),
            // say: <cid> <name>: <text>
            re_say: Regex::new(
                r#"^say:\s*(?P<cid>\d+)\s+(?P<name>.+?):\s*(?P<text>.+)$"#
            ).unwrap(),
            // sayteam: <cid> <name>: <text>
            re_sayteam: Regex::new(
                r#"^sayteam:\s*(?P<cid>\d+)\s+(?P<name>.+?):\s*(?P<text>.+)$"#
            ).unwrap(),
            re_client_connect: Regex::new(r"^ClientConnect:\s+(?P<cid>\d+)$").unwrap(),
            re_client_disconnect: Regex::new(r"^ClientDisconnect:\s+(?P<cid>\d+)$").unwrap(),
            re_client_begin: Regex::new(r"^ClientBegin:\s+(?P<cid>\d+)$").unwrap(),
            // ClientUserinfo: <cid> <infostring>
            re_client_userinfo: Regex::new(
                r"^ClientUserinfo:\s+(?P<cid>\d+)\s+(?P<info>.+)$"
            ).unwrap(),
            // ClientUserinfoChanged: <cid> <infostring>
            re_client_userinfo_changed: Regex::new(
                r"^ClientUserinfoChanged:\s+(?P<cid>\d+)\s+(?P<info>.+)$"
            ).unwrap(),
            // InitGame: \key\value\key\value...
            re_init_game: Regex::new(r"^InitGame:\s*(?P<data>.+)$").unwrap(),
            re_shutdown_game: Regex::new(r"^ShutdownGame:").unwrap(),
            re_warmup: Regex::new(r"^Warmup:$").unwrap(),
            re_init_round: Regex::new(r"^InitRound:\s*(?P<data>.*)$").unwrap(),
            // Flag: <cid> <action>: <name>
            re_flag: Regex::new(
                r"^Flag:\s+(?P<cid>\d+)\s+(?P<action>\d+):\s+(?P<name>.+)$"
            ).unwrap(),
            // Bomb: <action> by <cid>
            re_bomb: Regex::new(
                r"^Bomb\s+(?P<action>\w+)\s+by\s+(?P<cid>\d+)$"
            ).unwrap(),
            re_survivor_winner: Regex::new(
                r"^SurvivorWinner:\s+(?P<side>\w+)$"
            ).unwrap(),
            // AccountValidated: <cid> - <name> - <auth_login> - <notoriety>
            re_account_validated: Regex::new(
                r"^AccountValidated:\s+(?P<cid>\d+)\s+-\s+(?P<name>.+?)\s+-\s+(?P<auth>.+?)\s+-\s+(?P<notoriety>.+)$"
            ).unwrap(),
            re_account_rejected: Regex::new(
                r"^AccountRejected:\s+(?P<cid>\d+)\s+-\s+(?P<name>.+?)\s+-\s+(?P<reason>.+)$"
            ).unwrap(),
            // Item: <cid> <item_name>
            re_item: Regex::new(
                r"^Item:\s+(?P<cid>\d+)\s+(?P<item>\S+)$"
            ).unwrap(),
        }
    }

    /// Strip the timestamp prefix (e.g., "  0:00 ") from a raw log line.
    fn strip_timestamp<'a>(&self, raw: &'a str) -> &'a str {
        match self.re_timestamp.find(raw) {
            Some(m) => &raw[m.end()..],
            None => raw,
        }
    }

    /// Parse a backslash-delimited info string (e.g., \key\value\key2\value2)
    fn parse_info_string(info: &str) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        let parts: Vec<&str> = info.split('\\').collect();
        // Skip first empty element if string starts with '\'
        let start = if parts.first().is_some_and(|s| s.is_empty()) { 1 } else { 0 };
        let mut i = start;
        while i + 1 < parts.len() {
            pairs.push((parts[i].to_string(), parts[i + 1].to_string()));
            i += 2;
        }
        pairs
    }

    /// Query player info via RCON `dumpuser <slot>`.
    /// Returns key-value pairs from the server's response.
    pub async fn dumpuser(&self, slot: &str) -> anyhow::Result<PlayerInfo> {
        let response = self.rcon.send(&format!("dumpuser {}", slot)).await?;
        let mut info = PlayerInfo {
            cid: slot.to_string(),
            ..Default::default()
        };

        for line in response.lines() {
            let trimmed = line.trim();
            // dumpuser output: "key                value"
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                let val = val.trim();
                match key {
                    "name" | "n" => info.name = val.to_string(),
                    "cl_guid" => info.guid = val.to_string(),
                    "ip" => {
                        // IP may include port: "1.2.3.4:27960"
                        info.ip = val.split(':').next().unwrap_or(val).to_string();
                    }
                    "team" | "t" => info.team = val.to_string(),
                    "auth" => info.auth = val.to_string(),
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    /// Parse structured player info from RCON `status` response.
    pub async fn get_status_players(&self) -> anyhow::Result<Vec<StatusPlayer>> {
        let response = self.rcon.send("status").await?;
        let mut players = Vec::new();
        let re_status = Regex::new(
            r"^\s*(?P<slot>\d+)\s+(?P<score>-?\d+)\s+(?P<ping>\d+)\s+(?P<name>.*?)\s+(?P<lastmsg>\d+)\s+(?P<address>\S+)\s+(?P<qport>\d+)\s+(?P<rate>\d+)$"
        ).unwrap();

        for line in response.lines() {
            if let Some(caps) = re_status.captures(line) {
                players.push(StatusPlayer {
                    slot: caps.name("slot").unwrap().as_str().to_string(),
                    score: caps.name("score").unwrap().as_str().to_string(),
                    ping: caps.name("ping").unwrap().as_str().to_string(),
                    name: caps.name("name").unwrap().as_str().trim().to_string(),
                    address: caps.name("address").unwrap().as_str().to_string(),
                });
            }
        }
        Ok(players)
    }
}

#[async_trait]
impl GameParser for UrbanTerrorParser {
    fn game_name(&self) -> &str {
        "iourt43"
    }

    fn commands(&self) -> &GameCommands {
        &self.commands
    }

    fn parse_line(&self, line: &LogLine) -> ParsedAction {
        let text = self.strip_timestamp(&line.clean);

        // --- Kill ---
        if let Some(caps) = self.re_kill.captures(text) {
            let weapon_id: u32 = caps.name("weapid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let acid: i64 = caps.name("acid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let mod_name = weapon_name(weapon_id);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_KILL") {
                let event = Event::new(
                    evt_id,
                    EventData::Kill {
                        weapon: mod_name.to_string(),
                        damage: 100.0,
                        damage_type: mod_name.to_string(),
                        hit_location: "none".to_string(),
                    },
                )
                .with_client(acid)
                .with_target(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- Hit (damage) ---
        if let Some(caps) = self.re_hit.captures(text) {
            let acid: i64 = caps.name("acid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let dmg: f32 = caps.name("dmg").and_then(|m| m.as_str().parse().ok()).unwrap_or(0.0);
            let hitloc_id: u32 = caps.name("hitloc").and_then(|m| m.as_str().parse().ok()).unwrap_or(99);
            let hitloc = hit_location_name(hitloc_id);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_DAMAGE") {
                let event = Event::new(
                    evt_id,
                    EventData::Kill {
                        weapon: String::new(),
                        damage: dmg,
                        damage_type: "MOD_HIT".to_string(),
                        hit_location: hitloc.to_string(),
                    },
                )
                .with_client(acid)
                .with_target(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- Say (public chat) ---
        if let Some(caps) = self.re_say.captures(text) {
            let msg = caps.name("text").map(|m| m.as_str()).unwrap_or("");
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_SAY") {
                let event = Event::new(evt_id, EventData::Text(msg.to_string()))
                    .with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- Sayteam (team chat) ---
        if let Some(caps) = self.re_sayteam.captures(text) {
            let msg = caps.name("text").map(|m| m.as_str()).unwrap_or("");
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_TEAM_SAY") {
                let event = Event::new(evt_id, EventData::Text(msg.to_string()))
                    .with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- ClientConnect ---
        if let Some(caps) = self.re_client_connect.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_CONNECT") {
                let event = Event::new(evt_id, EventData::Empty).with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- ClientDisconnect ---
        if let Some(caps) = self.re_client_disconnect.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_DISCONNECT") {
                let event = Event::new(evt_id, EventData::Empty).with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- ClientBegin ---
        if let Some(caps) = self.re_client_begin.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_JOIN") {
                let event = Event::new(evt_id, EventData::Empty).with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- ClientUserinfo ---
        if let Some(caps) = self.re_client_userinfo.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let info = caps.name("info").map(|m| m.as_str()).unwrap_or("");
            let pairs = Self::parse_info_string(info);
            let info_json = serde_json::to_string(&pairs).unwrap_or_default();
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_INFO_CHANGE") {
                let event = Event::new(evt_id, EventData::Text(info_json)).with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- ClientUserinfoChanged ---
        if let Some(caps) = self.re_client_userinfo_changed.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let info = caps.name("info").map(|m| m.as_str()).unwrap_or("");
            let pairs = Self::parse_info_string(info);
            let info_json = serde_json::to_string(&pairs).unwrap_or_default();
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_INFO_CHANGE") {
                let event = Event::new(evt_id, EventData::Text(info_json)).with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- InitGame (map change / new game) ---
        if let Some(caps) = self.re_init_game.captures(text) {
            let data = caps.name("data").map(|m| m.as_str()).unwrap_or("");
            let pairs = Self::parse_info_string(data);
            let mut map_name = String::new();
            for (k, v) in &pairs {
                if k == "mapname" {
                    map_name = v.clone();
                    break;
                }
            }
            if let Some(evt_id) = self.event_registry.get_id("EVT_GAME_MAP_CHANGE") {
                let event = Event::new(
                    evt_id,
                    EventData::MapChange {
                        old: None,
                        new: map_name,
                    },
                );
                return ParsedAction::Event(event);
            }
        }

        // --- ShutdownGame ---
        if self.re_shutdown_game.is_match(text) {
            if let Some(evt_id) = self.event_registry.get_id("EVT_GAME_EXIT") {
                return ParsedAction::Event(Event::new(evt_id, EventData::Empty));
            }
        }

        // --- Warmup ---
        if self.re_warmup.is_match(text) {
            if let Some(evt_id) = self.event_registry.get_id("EVT_GAME_WARMUP") {
                return ParsedAction::Event(Event::new(evt_id, EventData::Empty));
            }
        }

        // --- InitRound ---
        if self.re_init_round.is_match(text) {
            if let Some(evt_id) = self.event_registry.get_id("EVT_GAME_ROUND_START") {
                return ParsedAction::Event(Event::new(evt_id, EventData::Empty));
            }
        }

        // --- Flag (CTF) ---
        if let Some(caps) = self.re_flag.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let action = caps.name("action").map(|m| m.as_str()).unwrap_or("0");
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_ACTION") {
                let event = Event::new(
                    evt_id,
                    EventData::Text(format!("flag:{}", action)),
                )
                .with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- Bomb ---
        if let Some(caps) = self.re_bomb.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let action = caps.name("action").map(|m| m.as_str()).unwrap_or("unknown");
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_ACTION") {
                let event = Event::new(
                    evt_id,
                    EventData::Text(format!("bomb:{}", action)),
                )
                .with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- SurvivorWinner ---
        if let Some(caps) = self.re_survivor_winner.captures(text) {
            let side = caps.name("side").map(|m| m.as_str()).unwrap_or("unknown");
            if let Some(evt_id) = self.event_registry.get_id("EVT_GAME_ROUND_END") {
                let event = Event::new(evt_id, EventData::Text(side.to_string()));
                return ParsedAction::Event(event);
            }
        }

        // --- AccountValidated (UrT auth system) ---
        if let Some(caps) = self.re_account_validated.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let auth = caps.name("auth").map(|m| m.as_str()).unwrap_or("");
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_AUTH") {
                let event = Event::new(evt_id, EventData::Text(auth.to_string()))
                    .with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // --- AccountRejected ---
        if let Some(caps) = self.re_account_rejected.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let reason = caps.name("reason").map(|m| m.as_str()).unwrap_or("");
            debug!(cid = cid, reason = reason, "Account rejected");
            return ParsedAction::NoOp;
        }

        // --- Item pickup ---
        if let Some(caps) = self.re_item.captures(text) {
            let cid: i64 = caps.name("cid").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let item = caps.name("item").map(|m| m.as_str()).unwrap_or("");
            if let Some(evt_id) = self.event_registry.get_id("EVT_CLIENT_ITEM_PICKUP") {
                let event = Event::new(evt_id, EventData::Text(item.to_string()))
                    .with_client(cid);
                return ParsedAction::Event(event);
            }
        }

        // Line not recognized
        if !text.is_empty() && !text.starts_with('-') {
            debug!(line = text, "Unrecognized log line");
        }
        ParsedAction::Unknown(line.raw.clone())
    }

    async fn get_map(&self) -> anyhow::Result<String> {
        let response = self.rcon.send("g_mapname").await?;
        // Response format: "g_mapname" is "ut4_mapname"
        if let Some(start) = response.rfind('"') {
            let before = &response[..start];
            if let Some(begin) = before.rfind('"') {
                return Ok(response[begin + 1..start].to_string());
            }
        }
        Ok(response.trim().to_string())
    }

    async fn get_player_list(&self) -> anyhow::Result<Vec<String>> {
        let players = self.get_status_players().await?;
        Ok(players.into_iter().map(|p| p.slot).collect())
    }

    async fn say(&self, message: &str) -> anyhow::Result<()> {
        let cmd = self.commands.say.replace("%(message)s", message);
        self.rcon.send(&cmd).await?;
        Ok(())
    }

    async fn message(&self, client_id: &str, message: &str) -> anyhow::Result<()> {
        let cmd = self
            .commands
            .message
            .replace("%(cid)s", client_id)
            .replace("%(message)s", message);
        self.rcon.send(&cmd).await?;
        Ok(())
    }

    async fn kick(&self, client_id: &str, reason: &str) -> anyhow::Result<()> {
        let cmd = self.commands.kick.replace("%(cid)s", client_id);
        self.rcon.send(&cmd).await?;
        info!(cid = client_id, reason = reason, "Player kicked");
        Ok(())
    }

    async fn ban(&self, client_id: &str, reason: &str) -> anyhow::Result<()> {
        let cmd = self.commands.ban.replace("%(cid)s", client_id);
        self.rcon.send(&cmd).await?;
        info!(cid = client_id, reason = reason, "Player banned");
        Ok(())
    }

    async fn temp_ban(&self, client_id: &str, reason: &str, duration_mins: u32) -> anyhow::Result<()> {
        let cmd = self.commands.tempban.replace("%(cid)s", client_id);
        self.rcon.send(&cmd).await?;
        info!(cid = client_id, reason = reason, mins = duration_mins, "Player temp-banned");
        Ok(())
    }

    async fn unban(&self, name: &str) -> anyhow::Result<()> {
        let cmd = self.commands.unban.replace("%(name)s", name);
        self.rcon.send(&cmd).await?;
        info!(name = name, "Player unbanned");
        Ok(())
    }

    async fn get_cvar(&self, name: &str) -> anyhow::Result<String> {
        let response = self.rcon.send(name).await?;
        Ok(response)
    }

    async fn set_cvar(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let cmd = self
            .commands
            .set_cvar
            .replace("%(name)s", name)
            .replace("%(value)s", value);
        self.rcon.send(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_log_line(text: &str) -> LogLine {
        LogLine {
            raw: text.to_string(),
            timestamp: None,
            clean: text.to_string(),
        }
    }

    fn make_parser() -> UrbanTerrorParser {
        let addr: std::net::SocketAddr = "127.0.0.1:27960".parse().unwrap();
        let rcon = Arc::new(RconClient::new(addr, "test"));
        let event_registry = Arc::new(EventRegistry::new());
        UrbanTerrorParser::new(rcon, event_registry)
    }

    fn assert_event_key(parser: &UrbanTerrorParser, action: &ParsedAction, expected_key: &str) {
        match action {
            ParsedAction::Event(evt) => {
                let key = parser.event_registry.get_key(evt.event_type).unwrap();
                assert_eq!(key, expected_key, "Expected event {expected_key}, got {key}");
            }
            other => panic!("Expected Event, got {:?}", other),
        }
    }

    // --- weapon_name / hit_location_name ---

    #[test]
    fn test_weapon_names() {
        assert_eq!(weapon_name(1), "UT_MOD_KNIFE");
        assert_eq!(weapon_name(15), "UT_MOD_AK103");
        assert_eq!(weapon_name(36), "UT_MOD_HEGRENADE");
        assert_eq!(weapon_name(999), "UT_MOD_UNKNOWN");
    }

    #[test]
    fn test_hit_location_names() {
        assert_eq!(hit_location_name(0), "HEAD");
        assert_eq!(hit_location_name(2), "TORSO");
        assert_eq!(hit_location_name(12), "LEFT_FOOT");
        assert_eq!(hit_location_name(99), "UNKNOWN");
    }

    // --- Kill ---

    #[test]
    fn test_parse_kill() {
        let parser = make_parser();
        let line = make_log_line("Kill: 3 5 15: PlayerA killed PlayerB by UT_MOD_AK103");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_KILL");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(3));
            assert_eq!(evt.target_id, Some(5));
            if let EventData::Kill { weapon, damage, .. } = &evt.data {
                assert_eq!(weapon, "UT_MOD_AK103");
                assert_eq!(*damage, 100.0);
            } else {
                panic!("Expected Kill data");
            }
        }
    }

    // --- Hit ---

    #[test]
    fn test_parse_hit() {
        let parser = make_parser();
        let line = make_log_line("Hit: 7 3 0 25: PlayerA hit PlayerB in the Head");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_DAMAGE");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(3));   // attacker
            assert_eq!(evt.target_id, Some(7));    // victim
            if let EventData::Kill { damage, hit_location, .. } = &evt.data {
                assert_eq!(*damage, 25.0);
                assert_eq!(hit_location, "HEAD");
            } else {
                panic!("Expected Kill data (used for hits too)");
            }
        }
    }

    // --- Say ---

    #[test]
    fn test_parse_say() {
        let parser = make_parser();
        let line = make_log_line("say: 2 PlayerName: hello world");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_SAY");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(2));
            if let EventData::Text(text) = &evt.data {
                assert_eq!(text, "hello world");
            } else {
                panic!("Expected Text data");
            }
        }
    }

    // --- Sayteam ---

    #[test]
    fn test_parse_sayteam() {
        let parser = make_parser();
        let line = make_log_line("sayteam: 4 TeamPlayer: go B!");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_TEAM_SAY");
    }

    // --- ClientConnect / Disconnect / Begin ---

    #[test]
    fn test_parse_client_connect() {
        let parser = make_parser();
        let line = make_log_line("ClientConnect: 12");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_CONNECT");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(12));
        }
    }

    #[test]
    fn test_parse_client_disconnect() {
        let parser = make_parser();
        let line = make_log_line("ClientDisconnect: 8");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_DISCONNECT");
    }

    #[test]
    fn test_parse_client_begin() {
        let parser = make_parser();
        let line = make_log_line("ClientBegin: 5");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_JOIN");
    }

    // --- InitGame (map change) ---

    #[test]
    fn test_parse_init_game() {
        let parser = make_parser();
        let line = make_log_line(r"InitGame: \mapname\ut4_turnpike\g_gametype\7\sv_maxclients\32");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_GAME_MAP_CHANGE");

        if let ParsedAction::Event(evt) = action {
            if let EventData::MapChange { new, old } = &evt.data {
                assert_eq!(new, "ut4_turnpike");
                assert!(old.is_none());
            } else {
                panic!("Expected MapChange data");
            }
        }
    }

    // --- ShutdownGame ---

    #[test]
    fn test_parse_shutdown_game() {
        let parser = make_parser();
        let line = make_log_line("ShutdownGame:");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_GAME_EXIT");
    }

    // --- Warmup ---

    #[test]
    fn test_parse_warmup() {
        let parser = make_parser();
        let line = make_log_line("Warmup:");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_GAME_WARMUP");
    }

    // --- Flag (CTF) ---

    #[test]
    fn test_parse_flag() {
        let parser = make_parser();
        let line = make_log_line("Flag: 3 1: PlayerA");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_ACTION");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(3));
            if let EventData::Text(text) = &evt.data {
                assert_eq!(text, "flag:1");
            }
        }
    }

    // --- Bomb ---

    #[test]
    fn test_parse_bomb() {
        let parser = make_parser();
        let line = make_log_line("Bomb planted by 6");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_ACTION");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(6));
            if let EventData::Text(text) = &evt.data {
                assert_eq!(text, "bomb:planted");
            }
        }
    }

    // --- SurvivorWinner ---

    #[test]
    fn test_parse_survivor_winner() {
        let parser = make_parser();
        let line = make_log_line("SurvivorWinner: Red");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_GAME_ROUND_END");

        if let ParsedAction::Event(evt) = action {
            if let EventData::Text(text) = &evt.data {
                assert_eq!(text, "Red");
            }
        }
    }

    // --- Item pickup ---

    #[test]
    fn test_parse_item() {
        let parser = make_parser();
        let line = make_log_line("Item: 2 ut_weapon_ak103");
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_ITEM_PICKUP");

        if let ParsedAction::Event(evt) = action {
            assert_eq!(evt.client_id, Some(2));
            if let EventData::Text(text) = &evt.data {
                assert_eq!(text, "ut_weapon_ak103");
            }
        }
    }

    // --- Timestamp stripping ---

    #[test]
    fn test_parse_with_timestamp() {
        let parser = make_parser();
        let line = LogLine {
            raw: "  3:45 ClientConnect: 9".to_string(),
            timestamp: Some("3:45".to_string()),
            clean: "  3:45 ClientConnect: 9".to_string(),
        };
        let action = parser.parse_line(&line);
        assert_event_key(&parser, &action, "EVT_CLIENT_CONNECT");
    }

    // --- Unrecognized line ---

    #[test]
    fn test_parse_unknown_line() {
        let parser = make_parser();
        let line = make_log_line("some random gibberish");
        let action = parser.parse_line(&line);
        assert!(matches!(action, ParsedAction::Unknown(_)));
    }

    // --- parse_info_string ---

    #[test]
    fn test_parse_info_string() {
        let pairs = UrbanTerrorParser::parse_info_string(r"\name\TestPlayer\team\red\ip\1.2.3.4");
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("name".to_string(), "TestPlayer".to_string()));
        assert_eq!(pairs[1], ("team".to_string(), "red".to_string()));
        assert_eq!(pairs[2], ("ip".to_string(), "1.2.3.4".to_string()));
    }
}
