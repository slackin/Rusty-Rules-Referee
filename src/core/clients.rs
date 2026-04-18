use std::collections::HashMap;
use tokio::sync::RwLock;

use super::Client;

/// In-memory manager for currently connected clients.
///
/// Tracks players by their game server slot ID (cid).
pub struct Clients {
    /// Map from slot ID (cid) to Client.
    by_cid: RwLock<HashMap<String, Client>>,
}

impl Clients {
    pub fn new() -> Self {
        Self {
            by_cid: RwLock::new(HashMap::new()),
        }
    }

    /// Get a clone of the client in the given slot.
    pub async fn get_by_cid(&self, cid: &str) -> Option<Client> {
        self.by_cid.read().await.get(cid).cloned()
    }

    /// Get a clone of the client with the given database ID.
    pub async fn get_by_id(&self, id: i64) -> Option<Client> {
        self.by_cid
            .read()
            .await
            .values()
            .find(|c| c.id == id)
            .cloned()
    }

    /// Find a client by partial name match (case-insensitive).
    pub async fn find_by_name(&self, name: &str) -> Vec<Client> {
        let lower = name.to_lowercase();
        self.by_cid
            .read()
            .await
            .values()
            .filter(|c| c.name.to_lowercase().contains(&lower))
            .cloned()
            .collect()
    }

    /// Add or update a client in a slot.
    pub async fn connect(&self, cid: &str, client: Client) {
        self.by_cid
            .write()
            .await
            .insert(cid.to_string(), client);
    }

    /// Update a client already in a slot using a closure.
    pub async fn update<F>(&self, cid: &str, f: F)
    where
        F: FnOnce(&mut Client),
    {
        if let Some(client) = self.by_cid.write().await.get_mut(cid) {
            f(client);
        }
    }

    /// Remove a client from a slot (on disconnect).
    pub async fn disconnect(&self, cid: &str) -> Option<Client> {
        self.by_cid.write().await.remove(cid)
    }

    /// Get all connected clients.
    pub async fn get_all(&self) -> Vec<Client> {
        self.by_cid.read().await.values().cloned().collect()
    }

    /// Number of connected clients.
    pub async fn count(&self) -> usize {
        self.by_cid.read().await.len()
    }

    /// Clear all connected clients (e.g., on map change).
    pub async fn clear(&self) {
        self.by_cid.write().await.clear();
    }
}

impl Default for Clients {
    fn default() -> Self {
        Self::new()
    }
}
