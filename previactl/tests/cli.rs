use std::fs;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use assert_cmd::prelude::*;
use tempfile::TempDir;

fn python3_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

fn write_fake_binary(path: &Path) {
    let script = r#"#!/bin/sh
python3 -u - <<'PY'
import os
import signal
from http.server import BaseHTTPRequestHandler, HTTPServer

address = os.environ.get("ADDRESS", "127.0.0.1")
port = int(os.environ.get("PORT", "0"))

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok")
        elif self.path == "/info":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"pid":1,"memoryBytes":0,"virtualMemoryBytes":0,"cpuUsagePercent":0.0}')
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, fmt, *args):
        return

httpd = HTTPServer((address, port), Handler)
print(f"fake node listening on {address}:{port}", flush=True)

def stop(_signum, _frame):
    httpd.shutdown()

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
httpd.serve_forever()
PY
"#;

    fs::write(path, script).expect("write fake binary");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn setup_previa_home() -> TempDir {
    let temp = TempDir::new().expect("tempdir");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    write_fake_binary(&bin.join("previa-main"));
    write_fake_binary(&bin.join("previa-runner"));
    temp
}

fn cargo_bin() -> Command {
    Command::cargo_bin("previactl").expect("previactl binary")
}

fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind free port")
        .local_addr()
        .expect("local addr")
        .port()
}

#[test]
fn dry_run_rejects_detach() {
    let temp = setup_previa_home();
    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["up", "--dry-run", "--detach"])
        .assert()
        .failure();
}

#[test]
fn dry_run_resolves_compose_without_writing_runtime() {
    let temp = setup_previa_home();
    let compose = temp.path().join("previa-compose.yaml");
    fs::write(
        &compose,
        r#"version: 1
main:
  address: 127.0.0.1
  port: 56100
runners:
  local:
    address: 127.0.0.1
    count: 1
    port_range:
      start: 56110
      end: 56110
"#,
    )
    .expect("write compose");

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["up", "--dry-run", compose.to_str().expect("compose str")])
        .assert()
        .success();

    assert!(!temp.path().join("stacks/default/run/state.json").exists());
}

#[test]
fn detached_lifecycle_supports_status_ps_logs_list_and_down() {
    if !python3_available() {
        return;
    }

    let temp = setup_previa_home();
    let stack = "itest";
    let main_port = find_free_port();
    let runner_port = find_free_port();

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args([
            "up",
            "--name",
            stack,
            "--detach",
            "--main-address",
            "127.0.0.1",
            "-p",
            &main_port.to_string(),
            "--runner-address",
            "127.0.0.1",
            "-P",
            &format!("{runner_port}:{runner_port}"),
            "-r",
            "1",
        ])
        .assert()
        .success();

    thread::sleep(Duration::from_millis(500));

    let status_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["status", "--name", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "running");
    assert_eq!(status_json["name"], stack);

    let ps_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["ps", "--name", stack, "--json"])
        .output()
        .expect("ps output");
    assert!(ps_output.status.success());
    let ps_json: serde_json::Value = serde_json::from_slice(&ps_output.stdout).expect("ps json");
    assert_eq!(ps_json.as_array().expect("ps array").len(), 2);

    let list_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["list", "--json"])
        .output()
        .expect("list output");
    assert!(list_output.status.success());
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("list json");
    assert_eq!(list_json.as_array().expect("list array")[0]["name"], stack);

    let logs_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["logs", "--name", stack, "--main"])
        .output()
        .expect("logs output");
    assert!(logs_output.status.success());
    let logs = String::from_utf8(logs_output.stdout).expect("utf8 logs");
    assert!(logs.contains("fake node listening"));

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["down", "--name", stack])
        .assert()
        .success();

    assert!(!temp.path().join("stacks").join(stack).join("run/state.json").exists());
}
