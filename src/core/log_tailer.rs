use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tracing::{debug, info, warn};

/// Continuously tails a game log file, yielding new lines as they appear.
/// Handles log rotation (file truncation or replacement) gracefully.
pub struct LogTailer {
    path: PathBuf,
    reader: Option<BufReader<File>>,
    position: u64,
    delay: std::time::Duration,
}

impl LogTailer {
    /// Create a new log tailer for the given file path.
    /// `delay` controls how often to poll for new data (e.g., 330ms).
    pub fn new(path: &Path, delay: std::time::Duration) -> Self {
        Self {
            path: path.to_path_buf(),
            reader: None,
            position: 0,
            delay,
        }
    }

    /// Open the file and seek to the end (only process new lines).
    pub async fn start(&mut self) -> anyhow::Result<()> {
        let file = File::open(&self.path).await?;
        let metadata = file.metadata().await?;
        let size = metadata.len();

        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::End(0)).await?;
        self.position = size;
        self.reader = Some(reader);

        info!(path = %self.path.display(), position = size, "Log tailer started at EOF");
        Ok(())
    }

    /// Read the next available line from the log file.
    /// Returns `None` if the file is not open or has been closed.
    /// Blocks (polls) until a line is available.
    pub async fn next_line(&mut self) -> Option<String> {
        loop {
            // Try to re-open if reader is gone (file was rotated / not yet opened)
            if self.reader.is_none() {
                match File::open(&self.path).await {
                    Ok(file) => {
                        let reader = BufReader::new(file);
                        self.reader = Some(reader);
                        self.position = 0;
                        info!(path = %self.path.display(), "Log file re-opened after rotation");
                    }
                    Err(_) => {
                        tokio::time::sleep(self.delay).await;
                        continue;
                    }
                }
            }

            let reader = self.reader.as_mut().unwrap();

            // Check for log rotation: file size smaller than our position
            if let Ok(metadata) = tokio::fs::metadata(&self.path).await {
                let current_size = metadata.len();
                if current_size < self.position {
                    warn!(
                        old_pos = self.position,
                        new_size = current_size,
                        "Log file truncated/rotated — reopening"
                    );
                    self.reader = None;
                    self.position = 0;
                    continue;
                }
            }

            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // No new data — wait and poll again
                    tokio::time::sleep(self.delay).await;
                    continue;
                }
                Ok(n) => {
                    self.position += n as u64;
                    let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
                    if !trimmed.is_empty() {
                        debug!(line = %trimmed, "Tailed line");
                        return Some(trimmed);
                    }
                    // Empty line — skip, keep reading
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "Error reading log file — will retry");
                    self.reader = None;
                    tokio::time::sleep(self.delay).await;
                    continue;
                }
            }
        }
    }
}
