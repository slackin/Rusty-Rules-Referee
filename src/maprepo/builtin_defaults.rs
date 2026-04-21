//! Hard-coded fallback defaults for the well-known stock Urban Terror 4.3
//! maps. These are used by `Storage::ensure_map_config` when neither a
//! `map_config_defaults` row nor an existing `map_configs` row is present
//! for a given map. The canonical seed still lives in migration 012; this
//! module is only a last-resort fallback so unknown maps still get a
//! sensible default.
//!
//! supported_gametypes is a CSV of gametype ids:
//!   0=FFA 1=LMS 3=TDM 4=TS 5=FTL 6=CAH 7=CTF 8=Bomb 9=Jump 10=FT 11=GunGame

/// (map_name, default_gametype, supported_gametypes)
const BUILTIN: &[(&str, &str, &str)] = &[
    ("ut4_abbey", "7", "0,3,4,7,8"),
    ("ut4_abaddon_rc8", "8", "3,4,7,8"),
    ("ut4_algiers", "7", "0,3,4,7,8"),
    ("ut4_austria", "7", "0,3,4,7,8"),
    ("ut4_bohemia", "8", "3,4,7,8"),
    ("ut4_casa", "7", "0,3,4,7,8"),
    ("ut4_cascade", "7", "0,3,4,7,8"),
    ("ut4_docks", "7", "0,3,4,7,8"),
    ("ut4_dressingroom", "3", "0,3,4"),
    ("ut4_eagle", "7", "0,3,4,7,8"),
    ("ut4_elgin", "7", "0,3,4,7,8"),
    ("ut4_firingrange", "3", "0,3,4"),
    ("ut4_ghosttown", "7", "0,3,4,7,8"),
    ("ut4_herring", "7", "0,3,4,7,8"),
    ("ut4_imperial_x13", "7", "0,3,4,7,8"),
    ("ut4_jumpents", "9", "9"),
    ("ut4_killroom", "0", "0,3,4"),
    ("ut4_kingdom", "7", "0,3,4,7,8"),
    ("ut4_kingpin", "7", "0,3,4,7,8"),
    ("ut4_mandolin", "7", "0,3,4,7,8"),
    ("ut4_mykonos_a17", "7", "0,3,4,7,8"),
    ("ut4_oildepot", "8", "3,4,7,8"),
    ("ut4_paris", "7", "0,3,4,7,8"),
    ("ut4_pipeline_b5", "7", "0,3,4,7,8"),
    ("ut4_prague", "7", "0,3,4,7,8"),
    ("ut4_prominence", "7", "0,3,4,7,8"),
    ("ut4_raiders", "8", "3,4,7,8"),
    ("ut4_ramelle", "7", "0,3,4,7,8"),
    ("ut4_ricochet", "3", "0,3,4"),
    ("ut4_riyadh", "7", "0,3,4,7,8"),
    ("ut4_sanc", "7", "0,3,4,7,8"),
    ("ut4_suburbs", "7", "0,3,4,7,8"),
    ("ut4_subway", "7", "0,3,4,7,8"),
    ("ut4_swim", "3", "0,3,4"),
    ("ut4_thingley", "7", "0,3,4,7,8"),
    ("ut4_tohunga_b8", "7", "0,3,4,7,8"),
    ("ut4_tombs", "7", "0,3,4,7,8"),
    ("ut4_turnpike", "7", "0,3,4,7,8"),
    ("ut4_uptown", "7", "0,3,4,7,8"),
];

/// Look up a hard-coded default for `map_name`.
/// Returns `(default_gametype, supported_gametypes)` on a hit.
pub fn builtin_default(map_name: &str) -> Option<(&'static str, &'static str)> {
    BUILTIN
        .iter()
        .find(|(name, _, _)| *name == map_name)
        .map(|(_, dgt, sgt)| (*dgt, *sgt))
}
