use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

// Handles one TCP client connection and returns a minimal HTTP response.
//
// This function intentionally keeps the protocol handling simple:
// - It reads up to 1024 bytes from the client and ignores the parsed request.
// - It always returns HTTP 200 with a fixed plain-text body.
// - It closes the connection after sending the response.
fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    // Small fixed-size buffer for the incoming HTTP request.
    // For this demo server, we only need enough data to consume the request.
    let mut buffer = [0_u8; 1024];

    // Read request bytes from the socket.
    // The returned byte count is not used here because this example does not
    // parse request headers or the request line.
    let _ = stream.read(&mut buffer)?;

    // Response body content.
    let body = "Hello from Rust on Windows!\n";

    // Build a valid HTTP/1.1 response.
    // - Status line: 200 OK
    // - Content-Type: plain text in UTF-8
    // - Content-Length: required by many clients for proper body framing
    // - Connection: close to keep lifecycle simple (one request per connection)
    //
    // Note: HTTP requires CRLF (\r\n) as line separators.
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    // Write the full response bytes to the client.
    stream.write_all(response.as_bytes())?;

    // Flush to ensure buffered bytes are pushed to the OS socket layer.
    stream.flush()?;

    // Returning Ok signals successful handling of this client.
    Ok(())
}

fn main() -> std::io::Result<()> {
    // Bind only to localhost so the server is accessible from this machine
    // and not exposed on the local network.
    let address = "127.0.0.1:8080";

    // Create a TCP listener socket. This fails if the port is in use or
    // permissions are insufficient.
    let listener = TcpListener::bind(address)?;

    // Helpful startup log for quick copy/paste into a browser.
    println!("Rust webserver listening at http://{address}");

    // Accept incoming connections forever.
    // listener.incoming() yields Result<TcpStream, std::io::Error> for each
    // accepted client connection.
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Process one client at a time, sequentially.
                // This keeps the example easy to follow, but limits throughput.
                // A production server would usually hand off work to threads
                // or async tasks.
                if let Err(err) = handle_connection(stream) {
                    // Log per-request errors and keep the server alive.
                    eprintln!("Request error: {err}");
                }
            }
            // If accepting a connection fails, report it and continue trying.
            Err(err) => eprintln!("Connection failed: {err}"),
        }
    }

    // This line is not expected to run in normal operation because the
    // incoming loop is endless until the process is terminated.
    Ok(())
}
