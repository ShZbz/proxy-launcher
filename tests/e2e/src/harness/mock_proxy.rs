use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockProxyMode {
    /// Responds with HTTP/1.1 204 No Content
    Http204,
    /// Responds with HTTP/1.1 502 Bad Gateway
    Http502,
    /// Responds with HTTP/1.1 with custom status code
    HttpStatus(u16),
    /// Closes socket immediately after TCP handshake
    CloseImmediately,
    /// Raw TCP echo / accept without HTTP response
    RawTcp,
}

/// Lightweight mock proxy server binding to an ephemeral loopback port (`127.0.0.1:0`).
pub struct MockProxyServer {
    port: u16,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockProxyServer {
    /// Starts a mock proxy server on `127.0.0.1:0` responding with HTTP 204 No Content.
    pub fn start_http_204() -> Self {
        Self::start(MockProxyMode::Http204)
    }

    /// Starts a mock proxy server on `127.0.0.1:0` responding with HTTP 502 Bad Gateway.
    pub fn start_http_502() -> Self {
        Self::start(MockProxyMode::Http502)
    }

    /// Starts a mock proxy server on `127.0.0.1:0` accepting TCP connections without HTTP payload.
    pub fn start_raw_tcp() -> Self {
        Self::start(MockProxyMode::RawTcp)
    }

    /// Starts a mock proxy server with the specified behavior mode.
    pub fn start(mode: MockProxyMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral mock port");
        let port = listener.local_addr().expect("Failed to get local_addr").port();
        let running = Arc::new(AtomicBool::new(true));

        let running_clone = running.clone();
        let handle = thread::spawn(move || {
            // Set 200ms accept timeout so thread can detect shutdown
            let _ = listener.set_nonblocking(false);

            while running_clone.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if !running_clone.load(Ordering::SeqCst) {
                            break;
                        }
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

                        match mode {
                            MockProxyMode::Http204 => {
                                let mut buf = [0u8; 1024];
                                let _ = stream.read(&mut buf);
                                let response = "HTTP/1.1 204 No Content\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                                let _ = stream.write_all(response.as_bytes());
                                let _ = stream.flush();
                            }
                            MockProxyMode::Http502 => {
                                let mut buf = [0u8; 1024];
                                let _ = stream.read(&mut buf);
                                let response = "HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                                let _ = stream.write_all(response.as_bytes());
                                let _ = stream.flush();
                            }
                            MockProxyMode::HttpStatus(code) => {
                                let mut buf = [0u8; 1024];
                                let _ = stream.read(&mut buf);
                                let response = format!("HTTP/1.1 {} Status\r\nConnection: close\r\nContent-Length: 0\r\n\r\n", code);
                                let _ = stream.write_all(response.as_bytes());
                                let _ = stream.flush();
                            }
                            MockProxyMode::CloseImmediately => {
                                drop(stream);
                            }
                            MockProxyMode::RawTcp => {
                                // Keep connection open briefly then drop
                                thread::sleep(Duration::from_millis(20));
                                drop(stream);
                            }
                        }
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        });

        Self {
            port,
            running,
            handle: Some(handle),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn host(&self) -> &str {
        "127.0.0.1"
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }
}

impl Drop for MockProxyServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Poke listener to unblock accept if blocked
        let _ = TcpStream::connect_timeout(&self.addr(), Duration::from_millis(50));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
