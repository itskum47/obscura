use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

#[cfg(feature = "stealth")]
use obscura_net::StealthHttpClient;
use obscura_net::{is_forbidden_ip, CookieJar, ObscuraHttpClient, SsrfGuardResolver};

/// Multi-hop redirect test:
/// Hop 1 (302) -> Hop 2 (302) -> Hop 3 (302) -> Final destination
async fn run_multi_hop_redirect_test(
    final_destination: &str,
    expect_scheme_err: bool,
    stealth: bool,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let final_dest = final_destination.to_string();

    tokio::spawn(async move {
        // Hop 1 -> Hop 2
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = "HTTP/1.1 302 Found\r\nLocation: /hop2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes()).await;
        }
        // Hop 2 -> Hop 3
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = "HTTP/1.1 302 Found\r\nLocation: /hop3\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes()).await;
        }
        // Hop 3 -> final_dest
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                final_dest
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });

    let start_url = Url::parse(&format!("http://{}/hop1", addr)).unwrap();

    #[cfg(feature = "stealth")]
    if stealth {
        let client = StealthHttpClient::with_proxy(Arc::new(CookieJar::new()), None, true);
        let result = client.fetch(&start_url).await;
        assert!(
            result.is_err(),
            "stealth multi-hop terminating at {final_destination} must fail"
        );
        let err_msg = result.unwrap_err().to_string();
        if expect_scheme_err {
            assert!(
                err_msg.contains("prohibited")
                    || err_msg.contains("non-HTTP")
                    || err_msg.contains("scheme"),
                "expected scheme rejection for {final_destination}, got: {err_msg}"
            );
        }
        return;
    }

    #[cfg(not(feature = "stealth"))]
    let _ = stealth;

    let client = ObscuraHttpClient::with_full_options(Arc::new(CookieJar::new()), None, true);
    client.set_allow_file_access(true); // Even with file access allowed, redirecting to file:// must fail closed
    let result = client.fetch(&start_url).await;
    assert!(
        result.is_err(),
        "multi-hop terminating at {final_destination} must fail"
    );
    let err_msg = result.unwrap_err().to_string();
    if expect_scheme_err {
        assert!(
            err_msg.contains("prohibited")
                || err_msg.contains("non-HTTP")
                || err_msg.contains("scheme"),
            "expected scheme rejection for {final_destination}, got: {err_msg}"
        );
    }
}

#[tokio::test]
async fn multi_hop_redirect_to_forbidden_schemes_and_private_targets() {
    for scheme_dest in [
        "file:///etc/passwd",
        "javascript:alert('pwned')",
        "data:text/html,<h1>bad</h1>",
    ] {
        run_multi_hop_redirect_test(scheme_dest, true, false).await;
    }
}

#[cfg(feature = "stealth")]
#[tokio::test]
async fn stealth_multi_hop_redirect_to_forbidden_schemes() {
    for scheme_dest in [
        "file:///etc/passwd",
        "javascript:alert('pwned')",
        "data:text/html,<h1>bad</h1>",
    ] {
        run_multi_hop_redirect_test(scheme_dest, true, true).await;
    }
}

#[test]
fn ssrf_mixed_dns_answers_fail_closed_validation() {
    // A domain returning both public and private IP addresses must fail closed.
    let public_v4 = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let loopback_v4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let metadata_v4 = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));
    let private_v4 = IpAddr::V4(Ipv4Addr::new(10, 0, 1, 5));
    let public_v6 = IpAddr::V6(Ipv6Addr::new(
        0x2606, 0x2800, 0x220, 0x1, 0x248, 0x1893, 0x25c8, 0x1946,
    ));
    let loopback_v6 = IpAddr::V6(Ipv6Addr::LOCALHOST);

    // Verify individual classification
    assert!(!is_forbidden_ip(public_v4));
    assert!(!is_forbidden_ip(public_v6));
    assert!(is_forbidden_ip(loopback_v4));
    assert!(is_forbidden_ip(metadata_v4));
    assert!(is_forbidden_ip(private_v4));
    assert!(is_forbidden_ip(loopback_v6));

    // Verify mixed socket address sets fail closed:
    let mixed_set_1 = [
        SocketAddr::new(public_v4, 80),
        SocketAddr::new(loopback_v4, 80),
    ];
    let bad_1 = mixed_set_1.iter().find(|sa| is_forbidden_ip(sa.ip()));
    assert!(
        bad_1.is_some(),
        "Mixed public + loopback must identify bad address"
    );
    assert_eq!(bad_1.unwrap().ip(), loopback_v4);

    let mixed_set_2 = [
        SocketAddr::new(public_v4, 443),
        SocketAddr::new(metadata_v4, 443),
    ];
    let bad_2 = mixed_set_2.iter().find(|sa| is_forbidden_ip(sa.ip()));
    assert!(
        bad_2.is_some(),
        "Mixed public + metadata must identify bad address"
    );
    assert_eq!(bad_2.unwrap().ip(), metadata_v4);

    let mixed_set_3 = [
        SocketAddr::new(public_v6, 443),
        SocketAddr::new(loopback_v6, 443),
    ];
    let bad_3 = mixed_set_3.iter().find(|sa| is_forbidden_ip(sa.ip()));
    assert!(
        bad_3.is_some(),
        "Mixed public IPv6 + loopback IPv6 must identify bad address"
    );
    assert_eq!(bad_3.unwrap().ip(), loopback_v6);
}

#[tokio::test]
async fn ssrf_resolver_blocks_localtest_me() {
    use reqwest::dns::{Name, Resolve};
    use std::str::FromStr;

    let resolver = SsrfGuardResolver::new(false);
    // localtest.me resolves to 127.0.0.1 in public DNS
    let result = resolver
        .resolve(Name::from_str("localtest.me").unwrap())
        .await;
    match result {
        Err(e) => {
            let err_str = e.to_string();
            assert!(
                err_str.contains("SSRF blocked"),
                "error message must indicate SSRF blocked: {err_str}"
            );
        }
        Ok(_) => panic!("SsrfGuardResolver must reject localtest.me resolving to 127.0.0.1"),
    }
}
