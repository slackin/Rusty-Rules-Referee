//! Persistent sync queue for offline resilience.
//!
//! When the client bot can't reach the master, events/penalties/stats are
//! queued in the local SQLite `sync_queue` table and drained when the
//! connection is restored.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tracing::{debug, error, warn};

use crate::storage::Storage;

/// Manages the persistent sync queue backed by the Storage trait.
pub struct SyncQueue {
    storage: Arc<dyn Storage>,
    server_id: Option<i64>,
}

impl SyncQueue {
    pub fn new(storage: Arc<dyn Storage>, server_id: Option<i64>) -> Self {
        Self { storage, server_id }
    }

    /// Enqueue a serializable item for sync with the master.
    pub async fn enqueue<T: Serialize>(
        &self,
        entity_type: &str,
        entity_id: Option<i64>,
        action: &str,
        data: &T,
    ) -> anyhow::Result<i64> {
        let payload = serde_json::to_string(data)?;
        let id = self.storage
            .enqueue_sync(entity_type, entity_id, action, &payload, self.server_id)
            .await?;
        debug!(id, entity_type, action, "Enqueued sync item");
        Ok(id)
    }

    /// Dequeue up to `limit` unsynced items, oldest first.
    pub async fn dequeue(&self, limit: u32) -> anyhow::Result<Vec<crate::core::SyncQueueEntry>> {
        let items = self.storage.dequeue_sync(limit).await?;
        Ok(items)
    }

    /// Mark items as successfully synced.
    pub async fn mark_synced(&self, ids: &[i64]) -> anyhow::Result<()> {
        if !ids.is_empty() {
            self.storage.mark_synced(ids).await?;
            debug!(count = ids.len(), "Marked sync items as synced");
        }
        Ok(())
    }

    /// Increment retry count for a failed item.
    pub async fn retry(&self, id: i64) -> anyhow::Result<()> {
        self.storage.retry_sync(id).await?;
        Ok(())
    }

    /// Remove old synced items to keep the queue table small.
    pub async fn prune(&self, older_than_days: u32) -> anyhow::Result<u64> {
        let count = self.storage.prune_synced(older_than_days).await?;
        if count > 0 {
            debug!(count, older_than_days, "Pruned old sync queue entries");
        }
        Ok(count)
    }

    /// Drain the queue: dequeue, send via callback, mark synced.
    /// Returns the number of items successfully sent.
    pub async fn drain<F, Fut>(&self, batch_size: u32, send_fn: F) -> anyhow::Result<u64>
    where
        F: Fn(crate::core::SyncQueueEntry) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<()>>,
    {
        let mut total_sent = 0u64;
        loop {
            let items = self.dequeue(batch_size).await?;
            if items.is_empty() {
                break;
            }

            let mut synced_ids = Vec::new();
            for item in items {
                let id = item.id;
                match send_fn(item).await {
                    Ok(()) => {
                        synced_ids.push(id);
                        total_sent += 1;
                    }
                    Err(e) => {
                        warn!(id, error = %e, "Failed to send queued item, will retry");
                        let _ = self.retry(id).await;
                        // Stop draining on first failure — master may be down
                        self.mark_synced(&synced_ids).await?;
                        return Ok(total_sent);
                    }
                }
            }
            self.mark_synced(&synced_ids).await?;
        }
        Ok(total_sent)
    }
}
