use std::fs;
use std::io::Write;
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
exec python3 -u - <<'PY'
import os
import signal
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

address = os.environ.get("ADDRESS", "127.0.0.1")
port = int(os.environ.get("PORT", "0"))
health_status = int(os.environ.get("HEALTH_STATUS", "200"))
health_status_file = os.environ.get("HEALTH_STATUS_FILE")

if os.environ.get("FAIL_STARTUP") == "1":
    sys.exit(1)

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            status = health_status
            if health_status_file and os.path.exists(health_status_file):
                with open(health_status_file, "r", encoding="utf-8") as fh:
                    status = int(fh.read().strip() or "200")
            self.send_response(status)
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
print(f"fake node listening on {address}:{port} pid={os.getpid()}", flush=True)

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
    assert!(status_json["main"].get("role").is_none());
    assert!(status_json["runners"][0].get("role").is_none());

    let ps_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["ps", "--name", stack, "--json"])
        .output()
        .expect("ps output");
    assert!(ps_output.status.success());
    let ps_json: serde_json::Value = serde_json::from_slice(&ps_output.stdout).expect("ps json");
    assert_eq!(ps_json.as_array().expect("ps array").len(), 2);
    assert_eq!(ps_json[0]["role"], "main");
    assert_eq!(ps_json[1]["role"], "runner");

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

#[test]
fn logs_supports_tail_count() {
    if !python3_available() {
        return;
    }

    let temp = setup_previa_home();
    let stack = "tailtest";
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

    let main_log = temp
        .path()
        .join("stacks")
        .join(stack)
        .join("logs")
        .join("main.log");
    fs::OpenOptions::new()
        .append(true)
        .open(&main_log)
        .expect("open main log")
        .write_all(b"line-one\nline-two\nline-three\n")
        .expect("append main log");

    let logs_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["logs", "--name", stack, "--main", "-t", "2"])
        .output()
        .expect("logs output");
    assert!(logs_output.status.success());
    let logs = String::from_utf8(logs_output.stdout).expect("utf8 logs");
    assert_eq!(logs, "line-two\nline-three\n");

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["down", "--name", stack])
        .assert()
        .success();
}

#[test]
fn status_reports_degraded_when_health_is_not_200() {
    if !python3_available() {
        return;
    }

    let temp = setup_previa_home();
    let stack = "healthcheck";
    let stack_config_dir = temp.path().join("stacks").join(stack).join("config");
    let health_status_file = temp.path().join("main-health-status.txt");
    fs::create_dir_all(&stack_config_dir).expect("stack config dir");
    fs::write(
        stack_config_dir.join("main.env"),
        format!(
            "ADDRESS=127.0.0.1\nPORT=5588\nRUNNER_ENDPOINTS=http://127.0.0.1:55880\nHEALTH_STATUS_FILE={}\n",
            health_status_file.display()
        ),
    )
    .expect("main env");

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
    fs::write(&health_status_file, "204\n").expect("health status file");

    let status_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["status", "--name", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "degraded");
    assert_eq!(status_json["main"]["state"], "degraded");
    assert_eq!(status_json["runners"][0]["state"], "running");

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["down", "--name", stack])
        .assert()
        .success();
}

#[test]
fn up_rejects_zero_ports_from_cli_and_compose() {
    let temp = setup_previa_home();

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["up", "--dry-run", "--main-port", "0"])
        .assert()
        .failure();

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["up", "--dry-run", "--runner-port-range", "0:56000"])
        .assert()
        .failure();

    let main_port_zero = temp.path().join("compose-main-port-zero.yaml");
    fs::write(
        &main_port_zero,
        r#"version: 1
main:
  port: 0
runners:
  local:
    count: 1
"#,
    )
    .expect("compose main port zero");

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args([
            "up",
            "--dry-run",
            main_port_zero.to_str().expect("compose path"),
        ])
        .assert()
        .failure();

    let runner_port_zero = temp.path().join("compose-runner-port-zero.yaml");
    fs::write(
        &runner_port_zero,
        r#"version: 1
runners:
  local:
    count: 1
    port_range:
      start: 0
      end: 56000
"#,
    )
    .expect("compose runner port zero");

    cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args([
            "up",
            "--dry-run",
            runner_port_zero.to_str().expect("compose path"),
        ])
        .assert()
        .failure();
}

#[test]
fn up_cleans_up_started_runners_when_later_startup_fails() {
    if !python3_available() {
        return;
    }

    let temp = setup_previa_home();
    let stack = "rollback";
    let main_port = find_free_port();
    let runner_port = find_free_port();
    let blocked_port = runner_port + 1;
    let listener = TcpListener::bind(("127.0.0.1", blocked_port)).expect("bind blocked port");

    let output = cargo_bin()
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
            &format!("{runner_port}:{blocked_port}"),
            "-r",
            "2",
        ])
        .output()
        .expect("up output");

    assert!(!output.status.success());

    drop(listener);
    thread::sleep(Duration::from_millis(500));

    let runner_log = temp
        .path()
        .join("stacks")
        .join(stack)
        .join("logs")
        .join("runners")
        .join(format!("{runner_port}.log"));
    let log_contents = fs::read_to_string(&runner_log).expect("runner log");
    assert!(log_contents.contains("fake node listening on"));

    let status_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["status", "--name", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "stopped");

    assert!(!temp.path().join("stacks").join(stack).join("run/state.json").exists());
    wait_for_logged_process_exit(&runner_log);
}

fn wait_for_logged_process_exit(path: &Path) {
    for _ in 0..30 {
        if logged_process_pid(path).is_some_and(|pid| !process_exists(pid)) {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("logged process still alive for '{}'", path.display());
}

fn logged_process_pid(path: &Path) -> Option<u32> {
    let contents = fs::read_to_string(path).ok()?;
    let line = contents
        .lines()
        .find(|line| line.contains("fake node listening on"))?;
    line.split(" pid=").nth(1)?.parse::<u32>().ok()
}

fn process_exists(pid: u32) -> bool {
    if !nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok() {
        return false;
    }
    let status_path = format!("/proc/{pid}/status");
    let Ok(status) = fs::read_to_string(status_path) else {
        return true;
    };
    !status.lines().any(|line| line.starts_with("State:\tZ"))
}
