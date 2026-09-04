use super::*;
use std::io::{Read, Write};

fn mock_registry(responses: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/package", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("mock registry accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buffer[..n]);
            }
            requests.push(
                String::from_utf8(request)
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_string(),
            );
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
        requests
    });
    (url, handle)
}

fn test_root(name: &str) -> PathBuf {
    env::temp_dir().join(format!("dsh-selection-{name}-{}", now_ms()))
}

fn tags(latest: &str, alpha: Option<&str>) -> String {
    json!({"dist-tags": {"latest": latest, "alpha": alpha}}).to_string()
}

fn seed_record(root: &Path, channel: &str, version: &str) -> RuntimeRecord {
    let destination = root.join("versions").join(format!("dsh-{version}"));
    fs::create_dir_all(&destination).unwrap();
    fs::create_dir_all(root.join("home")).unwrap();
    fs::create_dir_all(root.join("workspace")).unwrap();
    fs::write(destination.join("node.exe"), b"fixture, never executed").unwrap();
    fs::write(destination.join("bin.js"), b"fixture, never executed").unwrap();
    let record = RuntimeRecord {
        schema_version: 1,
        id: format!("managed-{version}"),
        dsh_version: version.into(),
        node_version: "24.20.0".into(),
        channel: channel.into(),
        recipe_id: "test".into(),
        node_path: destination.join("node.exe").display().to_string(),
        dsh_entry: destination.join("bin.js").display().to_string(),
        dsh_home: root.join("home").display().to_string(),
        workspace: root.join("workspace").display().to_string(),
        package_integrity: "sha512-fixture".into(),
        managed: true,
        smoke_tested: true,
        installed_at_ms: now_ms(),
    };
    atomic_write_json(&destination.join("runtime.json"), &record).unwrap();
    record
}

#[test]
fn registry_drift_missing_tags_and_errors_stop_before_any_install_side_effects() {
    let client = Client::builder().no_proxy().build().unwrap();
    for channel in ["recommended", "alpha"] {
        let initial = tags("1.2.3", Some("1.2.4-alpha.1"));
        for (index, second) in [
            (200, tags("2.0.0", Some("2.0.0-alpha.1"))),
            (200, tags("0.9.0", Some("0.9.0-alpha.1"))),
            (200, "{\"dist-tags\":{}}".into()),
            (200, tags("1.2.3", None)),
            (503, "{}".into()),
        ]
        .into_iter()
        .enumerate()
        {
            if channel == "recommended" && index == 3 {
                continue;
            }
            let root = test_root(&format!("drift-{channel}-{index}"));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("active.json"), b"old-active-evidence").unwrap();
            fs::write(root.join("previous.json"), b"old-previous-evidence").unwrap();
            let (url, server) = mock_registry(vec![(200, initial.clone()), second]);
            let (latest, alpha) = checked_versions_from(&client, &url).unwrap();
            let expected = if channel == "recommended" {
                latest
            } else {
                alpha.unwrap()
            };
            let result = install_runtime_from_registry(
                &root,
                &root.join("cache"),
                &root.join("home"),
                &root.join("workspace"),
                channel,
                &expected,
                |_, _| {},
                |_| panic!("must not reach smoke/npm install"),
                &client,
                &url,
            );
            assert!(result.is_err());
            assert_eq!(
                server.join().unwrap(),
                vec!["GET /package HTTP/1.1", "GET /package HTTP/1.1"]
            );
            assert_eq!(
                fs::read(root.join("active.json")).unwrap(),
                b"old-active-evidence"
            );
            assert_eq!(
                fs::read(root.join("previous.json")).unwrap(),
                b"old-previous-evidence"
            );
            assert_eq!(
                fs::read_dir(&root).unwrap().count(),
                2,
                "no staging, cache, Node, or installed version"
            );
            fs::remove_dir_all(&root).unwrap();
        }
    }
}

#[test]
fn exact_manifest_and_cached_record_must_match_confirmed_version_for_both_channels() {
    let client = Client::builder().no_proxy().build().unwrap();
    for (channel, expected) in [("recommended", "1.2.3"), ("alpha", "1.2.4-alpha.1")] {
        for scenario in ["valid", "wrong-manifest", "wrong-record"] {
            let root = test_root(&format!("exact-{channel}-{scenario}"));
            let mut record = seed_record(&root, channel, expected);
            if scenario == "wrong-record" {
                record.dsh_version = "9.9.9".into();
                atomic_write_json(
                    &root
                        .join("versions")
                        .join(format!("dsh-{expected}"))
                        .join("runtime.json"),
                    &record,
                )
                .unwrap();
            }
            let manifest_version = if scenario == "wrong-manifest" {
                "9.9.9"
            } else {
                expected
            };
            let (url, server) = mock_registry(vec![
                (200, tags("1.2.3", Some("1.2.4-alpha.1"))),
                (
                    200,
                    json!({"version": manifest_version, "dist": {"integrity": "sha512-fixture"}})
                        .to_string(),
                ),
            ]);
            let result = install_runtime_from_registry(
                &root,
                &root.join("cache"),
                &root.join("home"),
                &root.join("workspace"),
                channel,
                expected,
                |_, _| {},
                |_| panic!("cached path must not install"),
                &client,
                &url,
            );
            assert_eq!(
                server.join().unwrap(),
                vec![
                    "GET /package HTTP/1.1".to_string(),
                    format!("GET /package/{expected} HTTP/1.1")
                ]
            );
            if scenario == "valid" {
                assert_eq!(result.unwrap().dsh_version, expected);
                let active = switch_to_expected(&root, &record.id, channel, expected).unwrap();
                assert_eq!(
                    read_record(&root.join("active.json")).unwrap().dsh_version,
                    expected
                );
                assert_eq!(active.dsh_version, expected);
            } else {
                assert!(result.is_err());
                assert!(!root.join("active.json").exists());
                assert!(!root.join("staging").exists());
            }
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn activation_mismatch_preserves_both_pointers_and_requires_exact_semver() {
    let root = test_root("activation-version-guard");
    let record = seed_record(&root, "recommended", "1.2.3");
    fs::write(root.join("active.json"), b"old-active").unwrap();
    fs::write(root.join("previous.json"), b"old-previous").unwrap();
    assert!(switch_to_expected(&root, &record.id, "recommended", "9.9.9").is_err());
    assert!(switch_to_expected(&root, &record.id, "alpha", "1.2.3").is_err());
    assert_eq!(fs::read(root.join("active.json")).unwrap(), b"old-active");
    assert_eq!(
        fs::read(root.join("previous.json")).unwrap(),
        b"old-previous"
    );
    for invalid in [
        "latest",
        "next",
        "alpha",
        "1",
        "1.2",
        "^1.2.3",
        "1.2.3 || 2.0.0",
    ] {
        assert!(validate_version(invalid).is_err(), "{invalid}");
    }
    fs::remove_dir_all(root).unwrap();
}
