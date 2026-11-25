//! Runs the `aether-mcp` binary the way a user does, with a temporary HOME and a stub
//! `systemctl` on PATH, so nothing touches the real user session.
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempRoot(PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn install_service(root: &Path, vault: &Path) {
    let path = format!(
        "{}:{}",
        root.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_aether-mcp"))
        .args(["service", "install", "--vault"])
        .arg(vault)
        .env("HOME", root.join("home"))
        .env("PATH", path)
        .env("NO_COLOR", "1")
        .output()
        .expect("run aether-mcp");
    assert!(
        output.status.success(),
        "service install failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn service_install_restarts_the_service_so_a_new_vault_takes_effect() {
    let root =
        TempRoot(std::env::temp_dir().join(format!("aether_mcp_service_{}", uuid::Uuid::new_v4())));
    let log = root.0.join("systemctl.log");
    for dir in ["home", "bin", "vault_a", "vault_b"] {
        fs::create_dir_all(root.0.join(dir)).expect("create dir");
    }
    let stub = root.0.join("bin").join("systemctl");
    fs::write(
        &stub,
        format!("#!/bin/sh\necho \"$*\" >> \"{}\"\n", log.display()),
    )
    .expect("write stub");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod stub");

    install_service(&root.0, &root.0.join("vault_a"));
    install_service(&root.0, &root.0.join("vault_b"));

    let unit = fs::read_to_string(root.0.join("home/.config/systemd/user/aether-mcp.service"))
        .expect("read unit");
    let vault_b = fs::canonicalize(root.0.join("vault_b")).expect("canonical vault");
    assert!(
        unit.contains(&format!("--vault \"{}\"", vault_b.display())),
        "got: {unit}"
    );
    let once = [
        "--user daemon-reload",
        "--user enable aether-mcp.service",
        "--user restart aether-mcp.service",
    ];
    let calls = fs::read_to_string(&log).expect("read systemctl log");
    assert_eq!(
        calls.lines().collect::<Vec<_>>(),
        [once, once].concat(),
        "each install must restart the service so it serves the new definition"
    );
}
