//! A std-only HTTP/1.1 server for the updater tests: one request per connection, `Content-Length`,
//! `Connection: close`. Bound to 127.0.0.1 on a free port.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub enum Route {
    /// Status code and body.
    Bytes(u16, Vec<u8>),
    /// Accept the connection and send nothing.
    Stall,
}

pub struct TestServer {
    port: u16,
    hits: Arc<Mutex<HashMap<String, usize>>>,
}

impl TestServer {
    pub fn start(routes: Vec<(&'static str, Route)>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let routes: Arc<HashMap<&'static str, Route>> = Arc::new(routes.into_iter().collect());
        let hits: Arc<Mutex<HashMap<String, usize>>> = Arc::default();
        let served = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes.clone();
                let served = served.clone();
                thread::spawn(move || serve(stream, &routes, &served));
            }
        });
        TestServer { port, hits }
    }

    /// `http://127.0.0.1:<port>`
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// How many requests were served for `path`.
    pub fn hits(&self, path: &str) -> usize {
        self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }
}

fn serve(stream: TcpStream, routes: &HashMap<&'static str, Route>, hits: &Mutex<HashMap<String, usize>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    // Drain the headers.
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {}
        }
    }
    *hits.lock().unwrap().entry(path.clone()).or_default() += 1;
    let mut stream = reader.into_inner();
    match routes.get(path.as_str()) {
        Some(Route::Bytes(status, body)) => {
            let head = format!("HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", reason(*status), body.len());
            let _ = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(body));
        }
        Some(Route::Stall) => thread::sleep(Duration::from_secs(60)),
        None => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    }
}
