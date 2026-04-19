use axum::{extract::State, response::IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

/// Command documentation entry.
struct CommandDoc {
    name: &'static str,
    syntax: &'static str,
    description: &'static str,
    level: &'static str,
    plugin: &'static str,
}

/// GET /api/v1/commands — list all available commands with documentation.
pub async fn list_commands(
    AuthUser(_claims): AuthUser,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let commands = get_all_commands();

    let docs: Vec<serde_json::Value> = commands
        .iter()
        .map(|cmd| {
            serde_json::json!({
                "name": cmd.name,
                "syntax": cmd.syntax,
                "description": cmd.description,
                "level": cmd.level,
                "plugin": cmd.plugin,
            })
        })
        .collect();

    Json(serde_json::json!({"commands": docs}))
}

/// Returns the complete command documentation for all plugins.
fn get_all_commands() -> Vec<CommandDoc> {
    vec![
        // ---- Admin plugin (Guest level) ----
        CommandDoc { name: "help", syntax: "!help", description: "Display available commands for your level", level: "Guest", plugin: "admin" },
        CommandDoc { name: "leveltest", syntax: "!leveltest [player]", description: "Show a player's admin level", level: "Guest", plugin: "admin" },
        CommandDoc { name: "time", syntax: "!time", description: "Show current server time", level: "Guest", plugin: "admin" },
        CommandDoc { name: "register", syntax: "!register", description: "Register yourself as a user", level: "Guest", plugin: "admin" },
        CommandDoc { name: "regme", syntax: "!regme", description: "Register yourself as a user (alias)", level: "Guest", plugin: "admin" },
        CommandDoc { name: "r3", syntax: "!r3", description: "Show bot version information", level: "Guest", plugin: "admin" },

        // ---- Admin plugin (User level) ----
        CommandDoc { name: "regulars", syntax: "!regulars", description: "List online regular players", level: "User", plugin: "admin" },
        CommandDoc { name: "rules", syntax: "!rules", description: "Display server rules", level: "User", plugin: "admin" },

        // ---- Admin plugin (Mod level) ----
        CommandDoc { name: "status", syntax: "!status", description: "Show server and player status", level: "Mod", plugin: "admin" },
        CommandDoc { name: "lookup", syntax: "!lookup <player>", description: "Look up a player's info", level: "Mod", plugin: "admin" },
        CommandDoc { name: "list", syntax: "!list", description: "List connected players", level: "Mod", plugin: "admin" },
        CommandDoc { name: "admins", syntax: "!admins", description: "List online admins", level: "Mod", plugin: "admin" },
        CommandDoc { name: "warn", syntax: "!warn <player> [reason]", description: "Warn a player", level: "Mod", plugin: "admin" },
        CommandDoc { name: "kick", syntax: "!kick <player> [reason]", description: "Kick a player from the server", level: "Mod", plugin: "admin" },
        CommandDoc { name: "find", syntax: "!find <player>", description: "Find a player by name", level: "Mod", plugin: "admin" },
        CommandDoc { name: "seen", syntax: "!seen <player>", description: "Show when a player was last seen", level: "Mod", plugin: "admin" },
        CommandDoc { name: "aliases", syntax: "!aliases <player>", description: "Show a player's name history", level: "Mod", plugin: "admin" },
        CommandDoc { name: "poke", syntax: "!poke <player>", description: "Send a nudge to a player", level: "Mod", plugin: "admin" },
        CommandDoc { name: "warns", syntax: "!warns <player>", description: "Show a player's warnings", level: "Mod", plugin: "admin" },
        CommandDoc { name: "warntest", syntax: "!warntest <reason>", description: "Test a warn reason keyword", level: "Mod", plugin: "admin" },
        CommandDoc { name: "warnremove", syntax: "!warnremove <player>", description: "Remove last warning from a player", level: "Mod", plugin: "admin" },
        CommandDoc { name: "warninfo", syntax: "!warninfo <player>", description: "Show details of a player's warnings", level: "Mod", plugin: "admin" },
        CommandDoc { name: "spank", syntax: "!spank <player> [reason]", description: "Slap/spank a player (kick with message)", level: "Mod", plugin: "admin" },
        CommandDoc { name: "notice", syntax: "!notice <player> <text>", description: "Add a notice to a player's record", level: "Mod", plugin: "admin" },
        CommandDoc { name: "clear", syntax: "!clear <player>", description: "Clear a player's warnings", level: "Mod", plugin: "admin" },

        // ---- Admin plugin (Admin level) ----
        CommandDoc { name: "mute", syntax: "!mute <player> [duration_secs]", description: "Mute a player (default 600 seconds)", level: "Admin", plugin: "admin" },
        CommandDoc { name: "unmute", syntax: "!unmute <player>", description: "Unmute a player", level: "Admin", plugin: "admin" },
        CommandDoc { name: "tempban", syntax: "!tempban <player> [duration] [reason]", description: "Temporarily ban a player", level: "Admin", plugin: "admin" },
        CommandDoc { name: "lastbans", syntax: "!lastbans [count]", description: "Show recent bans", level: "Admin", plugin: "admin" },
        CommandDoc { name: "baninfo", syntax: "!baninfo <player>", description: "Show ban information for a player", level: "Admin", plugin: "admin" },
        CommandDoc { name: "spam", syntax: "!spam <keyword>", description: "Send a predefined spam message", level: "Admin", plugin: "admin" },
        CommandDoc { name: "spams", syntax: "!spams", description: "List available spam keywords", level: "Admin", plugin: "admin" },
        CommandDoc { name: "clientinfo", syntax: "!clientinfo <player>", description: "Show detailed client information", level: "Admin", plugin: "admin" },

        // ---- Admin plugin (Senior Admin level) ----
        CommandDoc { name: "ban", syntax: "!ban <player> [reason]", description: "Permanently ban a player", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "unban", syntax: "!unban <player>", description: "Remove a player's ban", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "permban", syntax: "!permban <player> [reason]", description: "Permanently ban a player (alias)", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "say", syntax: "!say <message>", description: "Send a server-wide message", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "longlist", syntax: "!longlist", description: "Detailed list of connected players", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "scream", syntax: "!scream <message>", description: "Send a big text message", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "warnclear", syntax: "!warnclear <player>", description: "Clear all warnings for a player", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "kickall", syntax: "!kickall", description: "Kick all non-admin players", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "banall", syntax: "!banall [reason]", description: "Ban all non-admin players", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "spankall", syntax: "!spankall [reason]", description: "Spank all non-admin players", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "mask", syntax: "!mask <player> <level>", description: "Mask a player's admin level", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "unmask", syntax: "!unmask <player>", description: "Remove admin level mask", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "makereg", syntax: "!makereg <player>", description: "Make a player a regular", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "unreg", syntax: "!unreg <player>", description: "Remove a player's regular status", level: "Senior Admin", plugin: "admin" },
        CommandDoc { name: "setnextmap", syntax: "!setnextmap <map>", description: "Set the next map", level: "Senior Admin", plugin: "admin" },

        // ---- Admin plugin (Super Admin level) ----
        CommandDoc { name: "putgroup", syntax: "!putgroup <player> <group>", description: "Assign a player to a group", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "ungroup", syntax: "!ungroup <player> <group>", description: "Remove a player from a group", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "maprotate", syntax: "!maprotate", description: "Force map rotation", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "maps", syntax: "!maps", description: "List available maps", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "nextmap", syntax: "!nextmap", description: "Show the next map", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "map", syntax: "!map <mapname>", description: "Change to a specific map", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "die", syntax: "!die", description: "Shut down the bot", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "restart", syntax: "!restart", description: "Restart the bot", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "reconfig", syntax: "!reconfig", description: "Reload configuration", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "rebuild", syntax: "!rebuild", description: "Rebuild the client list from server", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "runas", syntax: "!runas <player> <command>", description: "Run a command as another player", level: "Super Admin", plugin: "admin" },
        CommandDoc { name: "iamgod", syntax: "!iamgod", description: "Claim super admin (first use only)", level: "Super Admin", plugin: "admin" },

        // ---- Power Admin URT ----
        CommandDoc { name: "balance", syntax: "!balance", description: "Balance teams", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "teams", syntax: "!teams", description: "Show team info", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "swap", syntax: "!swap <player1> <player2>", description: "Swap two players between teams", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "force", syntax: "!force <player> <red|blue|spec|free>", description: "Force a player to a team", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "nuke", syntax: "!nuke <player>", description: "Nuke (slap) a player", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "slap", syntax: "!slap <player>", description: "Slap a player", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "veto", syntax: "!veto", description: "Veto the current vote", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "forcevote", syntax: "!forcevote <yes|no>", description: "Force pass or fail the current vote", level: "Admin", plugin: "poweradminurt" },
        CommandDoc { name: "shuffleteams", syntax: "!shuffleteams", description: "Shuffle teams randomly", level: "Admin", plugin: "poweradminurt" },
        CommandDoc { name: "ident", syntax: "!ident <player>", description: "Show player identity info (auth, IP)", level: "Mod", plugin: "poweradminurt" },
        CommandDoc { name: "gear", syntax: "!gear <gear_string>", description: "Set allowed gear for the server", level: "Senior Admin", plugin: "poweradminurt" },
        CommandDoc { name: "muteall", syntax: "!muteall", description: "Toggle global mute", level: "Admin", plugin: "poweradminurt" },

        // ---- Stats / XLR ----
        CommandDoc { name: "stats", syntax: "!stats [player]", description: "Show kill/death stats for current round", level: "Guest", plugin: "stats" },
        CommandDoc { name: "topstats", syntax: "!topstats", description: "Show top players this round", level: "Guest", plugin: "stats" },
        CommandDoc { name: "xlrstats", syntax: "!xlrstats [player]", description: "Show XLR skill rating and stats", level: "Guest", plugin: "xlrstats" },
        CommandDoc { name: "xlrtopstats", syntax: "!xlrtopstats", description: "Show top XLR-rated players", level: "Guest", plugin: "xlrstats" },

        // ---- Custom Commands ----
        CommandDoc { name: "discord", syntax: "!discord", description: "Show Discord server link (customizable)", level: "Guest", plugin: "customcommands" },

        // ---- Login ----
        CommandDoc { name: "login", syntax: "!login <password>", description: "Authenticate as admin", level: "Mod", plugin: "login" },

        // ---- Follow ----
        CommandDoc { name: "follow", syntax: "!follow <player>", description: "Get notifications about a player's activity", level: "Mod", plugin: "follow" },
        CommandDoc { name: "unfollow", syntax: "!unfollow <player>", description: "Stop following a player", level: "Mod", plugin: "follow" },
    ]
}
