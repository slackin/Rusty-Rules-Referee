use chrono::{DateTime, Utc};

/// Represents the current game state on the server.
/// Equivalent to Python B3's `Game` class.
#[derive(Debug, Clone)]
pub struct Game {
    pub game_name: String,
    pub game_type: Option<String>,
    pub mod_name: Option<String>,

    pub map_name: Option<String>,
    pub map_time_start: Option<DateTime<Utc>>,
    pub round_time_start: Option<DateTime<Utc>>,

    pub capture_limit: Option<u32>,
    pub frag_limit: Option<u32>,
    pub time_limit: Option<u32>,

    pub rounds: u32,
}

impl Game {
    pub fn new(game_name: &str) -> Self {
        let now = Utc::now();
        Self {
            game_name: game_name.to_string(),
            game_type: None,
            mod_name: None,
            map_name: None,
            map_time_start: Some(now),
            round_time_start: Some(now),
            capture_limit: None,
            frag_limit: None,
            time_limit: None,
            rounds: 0,
        }
    }

    /// Start a new round, incrementing the counter and resetting the timer.
    pub fn start_round(&mut self) {
        self.rounds += 1;
        self.round_time_start = Some(Utc::now());
    }

    /// Start a new map.
    pub fn start_map(&mut self, map_name: &str) {
        self.map_name = Some(map_name.to_string());
        self.map_time_start = Some(Utc::now());
        self.rounds = 0;
        self.start_round();
    }

    /// Elapsed time since map start, in seconds.
    pub fn map_time(&self) -> Option<i64> {
        self.map_time_start
            .map(|start| (Utc::now() - start).num_seconds())
    }

    /// Elapsed time since round start, in seconds.
    pub fn round_time(&self) -> Option<i64> {
        self.round_time_start
            .map(|start| (Utc::now() - start).num_seconds())
    }
}
