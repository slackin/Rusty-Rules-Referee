use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// RCON client for Quake 3 engine based games (UDP protocol).
/// Equivalent to Python B3's `b3.parsers.q3a.rcon.Rcon`.
///
/// Protocol: send `\xFF\xFF\xFF\xFFrcon "password" command\n`
///           recv `\xFF\xFF\xFF\xFFprint\n<response>`
pub struct RconClient {
    host: SocketAddr,
    password: String,
    socket: Mutex<Option<UdpSocket>>,
    socket_timeout: Duration,
}

/// The Q3A RCON packet header.
const RCON_HEADER: &[u8; 4] = b"\xFF\xFF\xFF\xFF";

impl RconClient {
    pub fn new(host: SocketAddr, password: &str) -> Self {
        Self {
            host,
            password: password.to_string(),
            socket: Mutex::new(None),
            socket_timeout: Duration::from_millis(800),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.socket_timeout = timeout;
        self
    }

    /// Ensure we have a connected UDP socket.
    async fn ensure_socket(&self) -> anyhow::Result<()> {
        let mut guard = self.socket.lock().await;
        if guard.is_none() {
            let socket = UdpSocket::bind("0.0.0.0:0").await?;
            socket.connect(self.host).await?;
            *guard = Some(socket);
        }
        Ok(())
    }

    /// Build an RCON packet: \xFF\xFF\xFF\xFF + rcon "password" command\n
    fn build_rcon_packet(&self, command: &str) -> Vec<u8> {
        let mut packet = Vec::with_capacity(32 + command.len());
        packet.extend_from_slice(RCON_HEADER);
        packet.extend_from_slice(b"rcon \"");
        packet.extend_from_slice(self.password.as_bytes());
        packet.extend_from_slice(b"\" ");
        packet.extend_from_slice(command.as_bytes());
        packet.push(b'\n');
        packet
    }

    /// Build a server query packet: \xFF\xFF\xFF\xFF + command\n
    fn build_query_packet(command: &str) -> Vec<u8> {
        let mut packet = Vec::with_capacity(8 + command.len());
        packet.extend_from_slice(RCON_HEADER);
        packet.extend_from_slice(command.as_bytes());
        packet.push(b'\n');
        packet
    }

    /// Send an RCON command and return the response.
    pub async fn send(&self, command: &str) -> anyhow::Result<String> {
        self.ensure_socket().await?;

        let packet = self.build_rcon_packet(command);

        let guard = self.socket.lock().await;
        let socket = guard.as_ref().unwrap();

        // Send the command
        socket.send(&packet).await?;

        // Receive the response with timeout
        let mut buf = vec![0u8; 4096];
        let mut response = String::new();

        match tokio::time::timeout(self.socket_timeout, socket.recv(&mut buf)).await {
            Ok(Ok(n)) => {
                let data = &buf[..n];
                // Strip the response header "\xFF\xFF\xFF\xFFprint\n"
                let header_len = 4 + 6; // 4 bytes \xFF + "print\n"
                if n > header_len && &data[..4] == RCON_HEADER {
                    response = String::from_utf8_lossy(&data[header_len..]).to_string();
                } else {
                    response = String::from_utf8_lossy(data).to_string();
                }
            }
            Ok(Err(e)) => {
                error!(error = %e, "RCON recv error");
                return Err(e.into());
            }
            Err(_) => {
                debug!(command = command, "RCON response timeout (may be normal for write commands)");
            }
        }

        Ok(response)
    }

    /// Send an RCON command without waiting for a response.
    pub async fn write(&self, command: &str) -> anyhow::Result<()> {
        self.ensure_socket().await?;

        let packet = self.build_rcon_packet(command);

        let guard = self.socket.lock().await;
        let socket = guard.as_ref().unwrap();
        socket.send(&packet).await?;
        debug!(command = command, "RCON command sent (no-wait)");
        Ok(())
    }

    /// Query the server (non-RCON query, e.g., getstatus).
    pub async fn query(&self, command: &str) -> anyhow::Result<String> {
        self.ensure_socket().await?;

        let packet = Self::build_query_packet(command);

        let guard = self.socket.lock().await;
        let socket = guard.as_ref().unwrap();
        socket.send(&packet).await?;

        let mut buf = vec![0u8; 4096];
        match tokio::time::timeout(self.socket_timeout, socket.recv(&mut buf)).await {
            Ok(Ok(n)) => {
                let data = &buf[..n];
                if n > 4 && &data[..4] == RCON_HEADER {
                    Ok(String::from_utf8_lossy(&data[4..]).to_string())
                } else {
                    Ok(String::from_utf8_lossy(data).to_string())
                }
            }
            Ok(Err(e)) => Err(e.into()),
            Err(_) => {
                warn!(command = command, "Server query timeout");
                Ok(String::new())
            }
        }
    }
}
