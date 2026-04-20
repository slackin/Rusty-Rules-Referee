use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::core::AuditEntry;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/players — connected players.
pub async fn list_players(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => return Json(serde_json::json!({"players": []})).into_response(),
    };
    let connected = ctx.clients.get_all().await;
    let groups = state.storage.get_groups().await.unwrap_or_default();

    let players: Vec<serde_json::Value> = connected.iter().map(|c| {
        let level = c.max_level();
        let group_name = groups.iter()
            .filter(|g| g.level <= level)
            .max_by_key(|g| g.level)
            .map(|g| g.name.clone());

        serde_json::json!({
            "id": c.id,
            "cid": c.cid,
            "name": c.name,
            "current_name": c.current_name,
            "auth": if c.auth.is_empty() { None } else { Some(&c.auth) },
            "guid": c.guid,
            "ip": c.ip.map(|ip| ip.to_string()),
            "team": format!("{:?}", c.team),
            "score": c.score,
            "ping": c.ping,
            "group_bits": c.group_bits,
            "group_name": group_name,
            "connected": c.connected,
        })
    }).collect();

    Json(serde_json::json!({"players": players})).into_response()
}

/// GET /api/v1/players/:id — player detail (reads from in-memory state, no RCON).
pub async fn get_player(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let client = match state.storage.get_client(id).await {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Player not found"}))).into_response();
        }
    };

    // Fetch all supplementary data in parallel
    let (aliases, penalties, xlr, groups) = tokio::join!(
        state.storage.get_aliases(id),
        state.storage.get_penalties(id, None),
        state.storage.get_xlr_player_stats(id),
        state.storage.get_groups(),
    );
    let connected = match state.ctx.as_ref() {
        Some(ctx) => ctx.clients.get_all().await,
        None => vec![],
    };
    let aliases = aliases.unwrap_or_default();
    let penalties = penalties.unwrap_or_default();
    let xlr = xlr.unwrap_or(None);
    let groups = groups.unwrap_or_default();
    let level = client.max_level();
    let group_name = groups.iter()
        .filter(|g| g.level <= level)
        .max_by_key(|g| g.level)
        .map(|g| g.name.clone());

    // Look up live data from in-memory connected clients (updated by background poller)
    let live_client = connected.iter().find(|c| c.id == id);
    let cid = live_client.map(|c| c.cid.clone());
    let team = live_client.map(|c| format!("{:?}", c.team));

    // Build live data from in-memory client state
    let live = live_client.map(|c| {
        let mut live = serde_json::json!({
            "score": c.score,
            "ping": c.ping,
        });
        if let Some(ref gear) = c.gear {
            live["gear"] = serde_json::json!(gear);
            live["loadout"] = decode_gear(gear);
        }
        if let Some(ref auth) = c.auth_name {
            live["auth"] = serde_json::json!(auth);
        }
        if let Some(ref cn) = c.current_name {
            live["current_name"] = serde_json::json!(cn);
        }
        if let Some(ref armband) = c.armband {
            live["armband"] = serde_json::json!(armband);
        }
        live
    });

    let mut response = serde_json::json!({
        "client": {
            "id": client.id,
            "cid": cid,
            "guid": client.guid,
            "name": client.name,
            "auth": if client.auth.is_empty() { None } else { Some(&client.auth) },
            "ip": client.ip.map(|ip| ip.to_string()),
            "group_bits": client.group_bits,
            "group_name": group_name,
            "team": team,
            "time_add": client.time_add,
            "time_edit": client.time_edit,
            "last_visit": client.last_visit,
        },
        "aliases": aliases,
        "penalties": penalties,
        "xlr_stats": xlr,
    });

    if let Some(live_data) = live {
        response["live"] = live_data;
    }

    Json(response).into_response()
}

/// Decode a UrT gear string (e.g. "GLAOWRA") into a structured loadout array.
/// Positions: 0=sidearm, 1=primary, 2=secondary nade, 3=nade, 4-6=items
fn decode_gear(gear: &str) -> serde_json::Value {
    let chars: Vec<char> = gear.chars().collect();
    let mut loadout = Vec::new();

    let slots = [
        (0, "Sidearm"),
        (1, "Primary"),
        (2, "Secondary"),
        (3, "Grenade"),
        (4, "Item 1"),
        (5, "Item 2"),
        (6, "Item 3"),
    ];

    for &(pos, slot_name) in &slots {
        if let Some(&code) = chars.get(pos) {
            if code == 'A' { continue; } // empty slot
            let name = gear_code_name(code);
            if name != "Unknown" {
                loadout.push(serde_json::json!({
                    "slot": slot_name,
                    "code": code.to_string(),
                    "name": name,
                    "category": gear_category(pos, code),
                }));
            }
        }
    }
    serde_json::json!(loadout)
}

/// Map a single gear character code to a weapon/item name.
fn gear_code_name(code: char) -> &'static str {
    match code {
        // Sidearms
        'F' => "Beretta 92FS",
        'G' => "Desert Eagle",
        'f' => "Glock",
        'g' => "Colt 1911",
        'k' => "FNP45",
        'l' => "Magnum",
        // Shotguns
        'H' => "SPAS-12",
        'j' => "Benelli M4",
        // SMGs
        'I' => "MP5K",
        'J' => "UMP45",
        'h' => "MAC-11",
        // Launchers
        'K' => "HK69",
        // Assault Rifles
        'L' => "LR-300ML",
        'M' => "G36",
        'a' => "AK-103",
        'c' => "Negev",
        'e' => "M4A1",
        // Snipers
        'N' => "PSG-1",
        'Z' => "SR-8",
        'i' => "FR-F1",
        // Grenades
        'O' => "HE Grenade",
        'Q' => "Smoke Grenade",
        // Items
        'R' => "Kevlar Vest",
        'S' => "NVGs",
        'T' => "Medkit",
        'U' => "Silencer",
        'V' => "Laser Sight",
        'W' => "Kevlar Helmet",
        'X' => "Extra Ammo",
        _ => "Unknown",
    }
}

/// Determine the gear category from slot position and code.
fn gear_category(pos: usize, code: char) -> &'static str {
    match pos {
        0 => "sidearm",
        1 => match code {
            'H' | 'j' => "shotgun",
            'I' | 'J' | 'h' | 'k' => "smg",
            'N' | 'Z' | 'i' => "sniper",
            'K' => "launcher",
            _ => "rifle",
        },
        2 | 3 => "grenade",
        _ => "item",
    }
}

#[derive(Deserialize)]
pub struct PlayerActionBody {
    pub reason: Option<String>,
    pub duration: Option<u32>,
}

/// POST /api/v1/players/:cid/kick
pub async fn kick_player(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<PlayerActionBody>,
) -> impl IntoResponse {
    let reason = body.reason.as_deref().unwrap_or("Kicked by admin");
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    match ctx.kick(&cid, reason).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "kick".to_string(),
                detail: format!("Kicked player cid={} reason={}", cid, reason),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            Json(serde_json::json!({"status": "ok"})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/v1/players/:cid/ban
pub async fn ban_player(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<PlayerActionBody>,
) -> impl IntoResponse {
    let reason = body.reason.as_deref().unwrap_or("Banned by admin");
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    let result = if let Some(duration) = body.duration {
        ctx.temp_ban(&cid, reason, duration).await
    } else {
        ctx.ban(&cid, reason).await
    };

    match result {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "ban".to_string(),
                detail: format!("Banned player cid={} duration={:?} reason={}", cid, body.duration, reason),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            Json(serde_json::json!({"status": "ok"})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/v1/players/:cid/message
pub async fn message_player(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    match ctx.message(&cid, msg).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/v1/players/:cid/mute — mute a player.
pub async fn mute_player(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<PlayerActionBody>,
) -> impl IntoResponse {
    let duration = body.duration.unwrap_or(600); // default 10 minutes
    let reason = body.reason.as_deref().unwrap_or("Muted by admin");
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    match ctx.write(&format!("mute {} {}", cid, duration)).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "mute".to_string(),
                detail: format!("Muted player cid={} duration={}s reason={}", cid, duration, reason),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            Json(serde_json::json!({"status": "ok", "duration": duration})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/v1/players/:cid/unmute — unmute a player.
pub async fn unmute_player(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> impl IntoResponse {
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    match ctx.write(&format!("unmute {}", cid)).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "unmute".to_string(),
                detail: format!("Unmuted player cid={}", cid),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            Json(serde_json::json!({"status": "ok"})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct ListClientsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

/// GET /api/v1/clients — paginated list of all clients from DB.
pub async fn list_all_clients(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListClientsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(25).min(100);
    let offset = query.offset.unwrap_or(0);
    let sort_by = query.sort_by.as_deref().unwrap_or("last_visit");
    let order = query.order.as_deref().unwrap_or("desc");

    let (clients, total) = match state.storage.list_clients(limit, offset, query.search.as_deref(), sort_by, order).await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let groups = state.storage.get_groups().await.unwrap_or_default();
    let connected = match state.ctx.as_ref() {
        Some(ctx) => ctx.clients.get_all().await,
        None => vec![],
    };

    let clients_json: Vec<serde_json::Value> = clients.iter().map(|c| {
        let level = c.max_level();
        let group_name = groups.iter()
            .filter(|g| g.level <= level)
            .max_by_key(|g| g.level)
            .map(|g| g.name.clone());
        let online = connected.iter().any(|cc| cc.id == c.id);
        let live_name = connected.iter().find(|cc| cc.id == c.id)
            .and_then(|cc| cc.current_name.clone());

        serde_json::json!({
            "id": c.id,
            "guid": c.guid,
            "name": c.name,
            "ip": c.ip.map(|ip| ip.to_string()),
            "auth": if c.auth.is_empty() { None } else { Some(&c.auth) },
            "current_name": live_name,
            "group_bits": c.group_bits,
            "group_name": group_name,
            "time_add": c.time_add,
            "last_visit": c.last_visit,
            "online": online,
        })
    }).collect();

    Json(serde_json::json!({
        "clients": clients_json,
        "total": total,
    })).into_response()
}

/// GET /api/v1/clients/search?q=name
pub async fn search_clients(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let q = query.q.as_deref().unwrap_or("");
    if q.is_empty() {
        return Json(serde_json::json!({"clients": []})).into_response();
    }

    let mut results = state.storage.find_clients(q).await.unwrap_or_default();
    let alias_results = state.storage.find_clients_by_alias(q).await.unwrap_or_default();

    // Merge, dedup by id
    for c in alias_results {
        if !results.iter().any(|r| r.id == c.id) {
            results.push(c);
        }
    }

    let clients: Vec<serde_json::Value> = results.iter().map(|c| {
        serde_json::json!({
            "id": c.id,
            "guid": c.guid,
            "name": c.name,
            "auth": if c.auth.is_empty() { None } else { Some(&c.auth) },
            "group_bits": c.group_bits,
            "last_visit": c.last_visit,
        })
    }).collect();

    Json(serde_json::json!({"clients": clients})).into_response()
}

#[derive(Deserialize)]
pub struct UpdateGroupBody {
    pub group_id: u64,
}

/// PUT /api/v1/players/:id/group — change a player's group.
pub async fn update_player_group(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateGroupBody>,
) -> impl IntoResponse {
    let group = match state.storage.get_group(body.group_id).await {
        Ok(g) => g,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Group not found"}))).into_response();
        }
    };

    let mut client = match state.storage.get_client(id).await {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Player not found"}))).into_response();
        }
    };

    client.group_bits = 1u64 << group.level;
    if let Err(e) = state.storage.save_client(&client).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let _ = state.storage.save_audit_entry(&AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "change_group".to_string(),
        detail: format!("Changed player id={} to group '{}' (level={})", id, group.name, group.level),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    Json(serde_json::json!({"status": "ok", "group_name": group.name, "group_bits": client.group_bits})).into_response()
}
