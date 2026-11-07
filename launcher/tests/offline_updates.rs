//! Update identity probes must stay read-only even when the executable embeds a complete game.
#![cfg(feature = "offline")]

#[test]
fn distribution_version_probe_does_not_initialize_player_data() {
    let root = std::env::temp_dir().join(format!(
        "horizon-offline-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut command = horizon_launcher::process::command(env!("CARGO_BIN_EXE_horizon-launcher"));
    command.arg("--offline-version");
    for name in [
        "HORIZON_HOME",
        "HORIZON_LAUNCHER_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
        "HORIZON_LAUNCHER_TRASH",
        "HORIZON_LAUNCHER_CLIPBOARD",
        "HORIZON_LAUNCHER_DESKTOP",
    ] {
        command.env(name, root.join(name));
    }
    command
        .env("HORIZON_LAUNCHER_LOCATION_FILE", root.join("location.json"))
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
        .env("HORIZON_NO_AUDIO", "1")
        .env("HORIZON_NO_KEYCHAIN", "1")
        .env("HORIZON_TELEMETRY_URL", "off");
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!(
            "horizon-offline {}",
            horizon_launcher::offline_runtime::VERSION
        )
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(&root).unwrap();
}
