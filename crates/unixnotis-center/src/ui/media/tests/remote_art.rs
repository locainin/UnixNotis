use std::net::IpAddr;
use std::process::Command;

use url::Url;

use super::{
    fetch_remote_art_output, read_remote_art, remote_art_command, resolve_argument,
    resolve_remote_target, stop_child, target_from_addresses, MAX_REMOTE_ADDRESSES,
    MAX_REMOTE_ART_BYTES,
};
use unixnotis_core::util::trusted_system_program_path;

fn url() -> Url {
    Url::parse("https://covers.example/art.png?size=large").expect("remote art URL")
}

#[test]
fn resolved_target_rejects_a_mixed_public_and_local_answer_set() {
    let addresses = vec![
        "93.184.216.34".parse::<IpAddr>().expect("public address"),
        "127.0.0.1".parse::<IpAddr>().expect("local address"),
    ];

    assert!(target_from_addresses(&url(), addresses, true).is_none());
}

#[test]
fn resolved_target_rejects_an_unusually_large_answer_set() {
    let addresses = (1..=17).map(|last| IpAddr::from([8, 8, 8, last])).collect();

    assert!(target_from_addresses(&url(), addresses, true).is_none());
}

#[test]
fn resolved_target_accepts_the_exact_address_limit_and_rejects_empty_answers() {
    let addresses = (1..=MAX_REMOTE_ADDRESSES)
        .map(|last| IpAddr::from([8, 8, 8, u8::try_from(last).expect("small address index")]))
        .collect::<Vec<_>>();

    let target = target_from_addresses(&url(), addresses.clone(), true)
        .expect("exact answer limit should be accepted");

    assert_eq!(target.addresses, addresses);
    assert!(target_from_addresses(&url(), Vec::new(), true).is_none());
}

#[test]
fn literal_remote_target_resolution_preserves_the_validated_address() {
    let url = Url::parse("https://93.184.216.34/art.png").expect("literal remote URL");

    let target = glib::MainContext::new()
        .block_on(resolve_remote_target(&url))
        .expect("public literal target");

    assert_eq!(target.addresses, vec![IpAddr::from([93, 184, 216, 34])]);
    assert!(!target.needs_resolve_override);

    let invalid = Url::parse("http://93.184.216.34/art.png").expect("HTTP URL");
    assert!(glib::MainContext::new()
        .block_on(resolve_remote_target(&invalid))
        .is_none());
}

#[cfg(unix)]
#[gtk::test]
fn remote_art_reader_runs_the_validated_request_and_returns_exact_output() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let program = std::env::temp_dir().join(format!(
        "unixnotis-center-art-transfer-{}-{stamp}",
        std::process::id()
    ));
    fs::write(&program, "#!/bin/sh\nprintf 'remote fixture'\n").expect("write transfer fixture");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700))
        .expect("set transfer fixture mode");
    let url = Url::parse("https://93.184.216.34/art.png").expect("literal remote URL");

    let bytes = glib::MainContext::new().block_on(read_remote_art(&url, &program));

    assert_eq!(bytes.as_deref(), Some(b"remote fixture".as_slice()));
    fs::remove_file(program).expect("remove transfer fixture");
}

#[test]
fn resolved_target_deduplicates_public_answers_for_pinning() {
    let address = "93.184.216.34".parse::<IpAddr>().expect("public address");

    let target =
        target_from_addresses(&url(), vec![address, address], true).expect("public target");

    assert_eq!(target.addresses, vec![address]);
    assert_eq!(
        resolve_argument(&target),
        "covers.example:443:93.184.216.34"
    );
}

#[test]
fn resolve_argument_brackets_ipv6_addresses() {
    let address = "2606:4700:4700::1111"
        .parse::<IpAddr>()
        .expect("public IPv6 address");
    let target = target_from_addresses(&url(), vec![address], true).expect("public target");

    assert_eq!(
        resolve_argument(&target),
        "covers.example:443:[2606:4700:4700::1111]"
    );
}

#[test]
fn curl_request_pins_dns_and_disables_proxy_and_redirects() {
    let address = "93.184.216.34".parse::<IpAddr>().expect("public address");
    let target = target_from_addresses(&url(), vec![address], true).expect("public target");
    let curl = trusted_system_program_path("curl").expect("trusted curl path");
    let command = remote_art_command(&target, &curl);
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(command.get_program(), curl);
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--resolve", "covers.example:443:93.184.216.34"]));
    assert!(args.windows(2).any(|pair| pair == ["--noproxy", "*"]));
    assert!(args.windows(2).any(|pair| pair == ["--proxy", ""]));
    assert!(args.windows(2).any(|pair| pair == ["--max-redirs", "0"]));
    assert!(args.windows(2).any(|pair| pair == ["--proto", "=https"]));
    assert!(args.contains(&"--location".to_string()));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--url", url().as_str()]));
}

fn shell_command(script: &str) -> Command {
    let shell = trusted_system_program_path("sh").expect("trusted shell path");
    let mut command = Command::new(shell);
    command.arg("-c").arg(script);
    command
}

#[test]
fn remote_art_output_accepts_exact_nonempty_success_only() {
    assert_eq!(MAX_REMOTE_ART_BYTES, 2 * 1_024 * 1_024);
    assert_eq!(
        fetch_remote_art_output(shell_command("printf 1234"), 4),
        Some(b"1234".to_vec())
    );
    assert_eq!(fetch_remote_art_output(shell_command("exit 0"), 4), None);
    assert_eq!(
        fetch_remote_art_output(shell_command("printf bad; exit 9"), 4),
        None
    );
}

#[test]
fn remote_art_output_rejects_one_byte_beyond_the_limit() {
    assert_eq!(
        fetch_remote_art_output(shell_command("printf 12345"), 4),
        None
    );
}

#[test]
fn stop_child_terminates_and_reaps_the_process() {
    let mut child = shell_command("sleep 30")
        .spawn()
        .expect("sleep child should start");

    stop_child(&mut child);

    assert!(child
        .try_wait()
        .expect("child state should remain readable")
        .is_some());
}
