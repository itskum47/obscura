use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

const OBSCURA: &str = env!("CARGO_BIN_EXE_obscura");

struct MockHttpServer {
    port: u16,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockHttpServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock HTTP server");
        let port = listener.local_addr().unwrap().port();
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        let handle = std::thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("set nonblocking on mock server");
            while r.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 4096];
                        if let Ok(n) = stream.read(&mut buf) {
                            let req = String::from_utf8_lossy(&buf[..n]);
                            if req.contains("GET /client1") {
                                let body = "<html><head><title>Client 1 Page</title></head><body><h1>Client 1 Page</h1></body></html>";
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\n\
                                     Content-Type: text/html\r\n\
                                     Content-Length: {}\r\n\
                                     Set-Cookie: client1_secret=secret_val_111; Path=/\r\n\
                                     Connection: close\r\n\r\n\
                                     {}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(resp.as_bytes());
                            } else if req.contains("GET /client2") {
                                let body = "<html><head><title>Client 2 Page</title></head><body><h1>Client 2 Page</h1></body></html>";
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\n\
                                     Content-Type: text/html\r\n\
                                     Content-Length: {}\r\n\
                                     Set-Cookie: client2_secret=secret_val_222; Path=/\r\n\
                                     Connection: close\r\n\r\n\
                                     {}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(resp.as_bytes());
                            } else {
                                let body = "<html><head><title>Default</title></head><body><h1>Default</h1></body></html>";
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\n\
                                     Content-Type: text/html\r\n\
                                     Content-Length: {}\r\n\
                                     Connection: close\r\n\r\n\
                                     {}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(resp.as_bytes());
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            port,
            running,
            handle: Some(handle),
        }
    }
}

impl Drop for MockHttpServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct TestMcpServer {
    port: u16,
    child: Child,
    token: String,
}

impl TestMcpServer {
    fn start(with_token: bool) -> Self {
        let ephemeral = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = ephemeral.local_addr().unwrap().port();
        drop(ephemeral);

        let token = "01234567890123456789012345678901".to_string();
        let mut cmd = Command::new(OBSCURA);
        cmd.args(["mcp", "--http", "--port", &port.to_string()])
            .env("OBSCURA_ALLOW_PRIVATE_NETWORK", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if with_token {
            cmd.env("OBSCURA_MCP_TOKEN", &token);
        }

        let child = cmd.spawn().expect("spawn obscura mcp --http");

        // Wait for server to accept TCP connections
        let target = format!("127.0.0.1:{}", port);
        let mut ready = false;
        for _ in 0..60 {
            if let Ok(mut stream) = TcpStream::connect(&target) {
                let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;
                let auth_hdr = if with_token {
                    format!("Authorization: Bearer {}\r\n", token)
                } else {
                    String::new()
                };
                let req = format!(
                    "POST /mcp HTTP/1.1\r\n\
                     Host: 127.0.0.1:{}\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     {}\
                     Connection: close\r\n\r\n\
                     {}",
                    port,
                    body.len(),
                    auth_hdr,
                    body
                );
                let _ = stream.write_all(req.as_bytes());
                let mut buf = Vec::new();
                let _ = stream.read_to_end(&mut buf);
                let res = String::from_utf8_lossy(&buf);
                if res.contains("200 OK") {
                    ready = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        assert!(ready, "MCP HTTP server failed to become ready on port {port}");

        Self { port, child, token }
    }

    fn post(&self, body: &str, session_id: Option<&str>, auth: Option<&str>) -> (u16, String, Option<String>) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", self.port)).expect("connect to MCP HTTP");
        let session_hdr = match session_id {
            Some(s) => format!("Mcp-Session-Id: {}\r\n", s),
            None => String::new(),
        };
        let auth_hdr = match auth {
            Some(a) => format!("Authorization: {}\r\n", a),
            None => String::new(),
        };
        let req = format!(
            "POST /mcp HTTP/1.1\r\n\
             Host: 127.0.0.1:{}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             {}\
             {}\
             Connection: close\r\n\r\n\
             {}",
            self.port,
            body.len(),
            session_hdr,
            auth_hdr,
            body
        );
        stream.write_all(req.as_bytes()).expect("write request");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).expect("read response");
        let resp = String::from_utf8_lossy(&buf).to_string();

        let status = resp
            .lines()
            .next()
            .and_then(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                parts.get(1)?.parse::<u16>().ok()
            })
            .unwrap_or(0);

        let mut out_session_id = None;
        for line in resp.lines() {
            if line.to_lowercase().starts_with("mcp-session-id:") {
                if let Some((_, v)) = line.split_once(':') {
                    out_session_id = Some(v.trim().to_string());
                }
            }
        }

        (status, resp, out_session_id)
    }

    fn delete(&self, session_id: Option<&str>, auth: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", self.port)).expect("connect to MCP HTTP");
        let session_hdr = match session_id {
            Some(s) => format!("Mcp-Session-Id: {}\r\n", s),
            None => String::new(),
        };
        let auth_hdr = match auth {
            Some(a) => format!("Authorization: {}\r\n", a),
            None => String::new(),
        };
        let req = format!(
            "DELETE /mcp HTTP/1.1\r\n\
             Host: 127.0.0.1:{}\r\n\
             {}\
             {}\
             Connection: close\r\n\r\n",
            self.port,
            session_hdr,
            auth_hdr
        );
        stream.write_all(req.as_bytes()).expect("write request");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).expect("read response");
        let resp = String::from_utf8_lossy(&buf).to_string();

        let status = resp
            .lines()
            .next()
            .and_then(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                parts.get(1)?.parse::<u16>().ok()
            })
            .unwrap_or(0);

        (status, resp)
    }
}

impl Drop for TestMcpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn test_mcp_http_session_isolation_and_ownership() {
    let mock = MockHttpServer::start();
    let server = TestMcpServer::start(true);
    let bearer = format!("Bearer {}", server.token);

    // 1. Client 1 initializes a session
    let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
    let (status1, resp1, sid1) = server.post(init_body, None, Some(&bearer));
    assert_eq!(status1, 200, "client 1 init failed: {resp1}");
    let sid1 = sid1.expect("client 1 should receive Mcp-Session-Id");
    assert!(!sid1.is_empty(), "session id 1 must not be empty");

    // 2. Client 2 initializes a completely independent session
    let (status2, resp2, sid2) = server.post(init_body, None, Some(&bearer));
    assert_eq!(status2, 200, "client 2 init failed: {resp2}");
    let sid2 = sid2.expect("client 2 should receive Mcp-Session-Id");
    assert_ne!(sid1, sid2, "two clients must receive distinct session IDs");

    // 3. Client 1 navigates to /client1 and receives client1 cookie
    let url1 = format!("http://127.0.0.1:{}/client1", mock.port);
    let nav_cmd1 = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"browser_navigate","arguments":{{"url":"{}"}}}}}}"#,
        url1
    );
    let (st_nav1, resp_nav1, _) = server.post(&nav_cmd1, Some(&sid1), Some(&bearer));
    assert_eq!(st_nav1, 200, "client 1 nav failed: {resp_nav1}");
    assert!(resp_nav1.contains("Client 1 Page"));

    // Verify client 1 has client1_secret cookie
    let cookies_cmd1 = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"browser_get_cookies","arguments":{}}}"#;
    let (st_c1, resp_c1, _) = server.post(cookies_cmd1, Some(&sid1), Some(&bearer));
    assert_eq!(st_c1, 200);
    assert!(resp_c1.contains("client1_secret"), "client 1 must have its own cookie: {resp_c1}");

    // 4. Client 2 inspects its cookies: MUST NOT have client 1's cookie!
    let (st_c2, resp_c2, _) = server.post(cookies_cmd1, Some(&sid2), Some(&bearer));
    assert_eq!(st_c2, 200);
    assert!(
        !resp_c2.contains("client1_secret"),
        "CRITICAL LEAK: Client 2 saw Client 1's cookie! {resp_c2}"
    );

    // 5. Client 2 navigates to /client2 and verifies independent state
    let url2 = format!("http://127.0.0.1:{}/client2", mock.port);
    let nav_cmd2 = format!(
        r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"browser_navigate","arguments":{{"url":"{}"}}}}}}"#,
        url2
    );
    let (st_nav2, resp_nav2, _) = server.post(&nav_cmd2, Some(&sid2), Some(&bearer));
    assert_eq!(st_nav2, 200, "client 2 nav failed: {resp_nav2}");
    assert!(resp_nav2.contains("Client 2 Page"));

    // Verify Client 2 has client2_secret and not client1_secret
    let (st_c2b, resp_c2b, _) = server.post(cookies_cmd1, Some(&sid2), Some(&bearer));
    assert_eq!(st_c2b, 200);
    assert!(resp_c2b.contains("client2_secret"));
    assert!(!resp_c2b.contains("client1_secret"), "Client 2 must not see Client 1 cookie");

    // Client 1 checks again: Client 1 must NOT see client2_secret!
    let (st_c1b, resp_c1b, _) = server.post(cookies_cmd1, Some(&sid1), Some(&bearer));
    assert_eq!(st_c1b, 200);
    assert!(resp_c1b.contains("client1_secret"));
    assert!(!resp_c1b.contains("client2_secret"), "Client 1 must not see Client 2 cookie");

    // 5b. Verify JavaScript Context and DOM isolation
    // Client 1 sets a global variable and DOM property
    let set_js1 = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"browser_evaluate","arguments":{"expression":"window.__SECRET_ISOLATION_TOKEN__ = 'c1_secret_987'; document.title"}}}"#;
    let (st_js1, resp_js1, _) = server.post(set_js1, Some(&sid1), Some(&bearer));
    assert_eq!(st_js1, 200);
    assert!(resp_js1.contains("Client 1 Page"));

    // Client 2 attempts to inspect Client 1's JavaScript context variable
    let check_js2 = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"browser_evaluate","arguments":{"expression":"String(typeof window.__SECRET_ISOLATION_TOKEN__)"}}}"#;
    let (st_js2, resp_js2, _) = server.post(check_js2, Some(&sid2), Some(&bearer));
    assert_eq!(st_js2, 200);
    assert!(
        resp_js2.contains("undefined"),
        "CRITICAL JS LEAK: Client 2 accessed Client 1 JS context! {resp_js2}"
    );

    // Client 2 evaluates its own DOM title: must be "Client 2 Page", not Client 1
    let check_dom2 = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"browser_evaluate","arguments":{"expression":"document.title"}}}"#;
    let (st_dom2, resp_dom2, _) = server.post(check_dom2, Some(&sid2), Some(&bearer));
    assert_eq!(st_dom2, 200);
    assert!(resp_dom2.contains("Client 2 Page"));
    assert!(!resp_dom2.contains("Client 1 Page"));

    // 5c. Verify Tab List isolation: Client 2 must not see Client 1's tabs
    let tabs_cmd = r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"browser_tab_list","arguments":{}}}"#;
    let (st_tabs2, resp_tabs2, _) = server.post(tabs_cmd, Some(&sid2), Some(&bearer));
    assert_eq!(st_tabs2, 200);
    assert!(resp_tabs2.contains("Client 2 Page"), "Client 2 must see its own tab: {resp_tabs2}");
    assert!(
        !resp_tabs2.contains("Client 1 Page"),
        "CRITICAL TAB LEAK: Client 2 saw Client 1's tabs! {resp_tabs2}"
    );

    // 6. Adversarial access:
    // Unauthorized client (no Bearer header) trying to access Client 1's session
    let (unauth_status, unauth_resp, _) = server.post(cookies_cmd1, Some(&sid1), None);
    assert_eq!(unauth_status, 401, "unauthorized access must be 401: {unauth_resp}");

    // Wrong Bearer token
    let bad_auth = "Bearer 99999999999999999999999999999999";
    let (bad_status, bad_resp, _) = server.post(cookies_cmd1, Some(&sid1), Some(bad_auth));
    assert_eq!(bad_status, 401, "bad token must be 401: {bad_resp}");

    // Non-existent session ID
    let (not_found_st, not_found_resp, _) = server.post(cookies_cmd1, Some("non-existent-session-id"), Some(&bearer));
    assert_eq!(not_found_st, 404, "non-existent session must return 404: {not_found_resp}");

    // Tool call without session ID header
    let (no_sid_st, no_sid_resp, _) = server.post(cookies_cmd1, None, Some(&bearer));
    assert_eq!(no_sid_st, 400, "tool call without session id must return 400: {no_sid_resp}");

    // 7. Session deletion via DELETE /mcp
    let (del_st, del_resp) = server.delete(Some(&sid1), Some(&bearer));
    assert_eq!(del_st, 204, "session deletion must be 204: {del_resp}");

    // Client 1 trying to access deleted session -> 404
    let (after_del_st, _, _) = server.post(cookies_cmd1, Some(&sid1), Some(&bearer));
    assert_eq!(after_del_st, 404, "deleted session must return 404");

    // Client 2 session still works after Client 1 deletion!
    let (c2_still_alive_st, c2_alive_resp, _) = server.post(cookies_cmd1, Some(&sid2), Some(&bearer));
    assert_eq!(c2_still_alive_st, 200);
    assert!(c2_alive_resp.contains("client2_secret"));
}

#[test]
fn test_mcp_http_concurrent_client_isolation() {
    let mock = MockHttpServer::start();
    let server = Arc::new(TestMcpServer::start(false));
    let mock_port = mock.port;

    // Spin up 4 concurrent client threads
    let mut handles = Vec::new();
    for client_idx in 0..4 {
        let s = server.clone();
        handles.push(std::thread::spawn(move || {
            let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
            let (status, _, sid) = s.post(init_body, None, None);
            assert_eq!(status, 200);
            let sid = sid.expect("session id");

            let url = format!("http://127.0.0.1:{}/client{}", mock_port, if client_idx % 2 == 0 { 1 } else { 2 });
            let nav_cmd = format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"browser_navigate","arguments":{{"url":"{}"}}}}}}"#,
                url
            );
            let (st, _, _) = s.post(&nav_cmd, Some(&sid), None);
            assert_eq!(st, 200);

            // Repeat commands to verify session state persistence
            let eval_cmd = format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"browser_evaluate","arguments":{{"expression":"'client_{}'"}}}}}}"#,
                client_idx
            );
            let (st_ev, resp_ev, _) = s.post(&eval_cmd, Some(&sid), None);
            assert_eq!(st_ev, 200);
            assert!(resp_ev.contains(&format!("client_{client_idx}")));

            // Cleanly delete session
            let (del_st, _) = s.delete(Some(&sid), None);
            assert_eq!(del_st, 204);
        }));
    }

    for h in handles {
        h.join().expect("thread join");
    }
}

#[test]
fn test_mcp_http_session_lifecycle_and_reclamation() {
    let server = Arc::new(TestMcpServer::start(false));
    let mut sessions = Vec::new();

    // Create 8 concurrent sessions
    for i in 0..8 {
        let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
        let (status, _, sid) = server.post(init_body, None, None);
        assert_eq!(status, 200, "session {i} initialize failed");
        let sid = sid.expect("session id");
        sessions.push(sid);
    }

    // Verify all 8 sessions have distinct IDs
    let mut unique_sids = sessions.clone();
    unique_sids.sort();
    unique_sids.dedup();
    assert_eq!(unique_sids.len(), 8, "all created session IDs must be unique");

    // Perform operations in each session
    for (i, sid) in sessions.iter().enumerate() {
        let eval_cmd = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"browser_evaluate","arguments":{{"expression":"'val_{i}'"}}}}}}"#
        );
        let (st, resp, _) = server.post(&eval_cmd, Some(sid), None);
        assert_eq!(st, 200);
        assert!(resp.contains(&format!("val_{i}")));
    }

    // Reclaim half the sessions (delete 0..4)
    for sid in &sessions[..4] {
        let (del_st, _) = server.delete(Some(sid), None);
        assert_eq!(del_st, 204, "session deletion must return 204");
    }

    // Verify deleted sessions return 404
    for sid in &sessions[..4] {
        let (st, _, _) = server.post(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"browser_tab_list","arguments":{}}}"#,
            Some(sid),
            None,
        );
        assert_eq!(st, 404, "deleted session must be gone");
    }

    // Verify remaining sessions (4..8) are completely unaffected
    for (i, sid) in sessions[4..].iter().enumerate() {
        let orig_idx = i + 4;
        let eval_cmd = format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"browser_evaluate","arguments":{{"expression":"'val_{orig_idx}'"}}}}}}"#
        );
        let (st, resp, _) = server.post(&eval_cmd, Some(sid), None);
        assert_eq!(st, 200);
        assert!(resp.contains(&format!("val_{orig_idx}")));
    }

    // Create new sessions in the reclaimed slots
    for i in 0..4 {
        let init_body = r#"{"jsonrpc":"2.0","id":5,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
        let (status, _, sid) = server.post(init_body, None, None);
        assert_eq!(status, 200, "reclaimed slot {i} creation failed");
        let sid = sid.expect("session id");
        let (del_st, _) = server.delete(Some(&sid), None);
        assert_eq!(del_st, 204);
    }

    // Clean up remaining sessions
    for sid in &sessions[4..] {
        let (del_st, _) = server.delete(Some(sid), None);
        assert_eq!(del_st, 204);
    }
}
