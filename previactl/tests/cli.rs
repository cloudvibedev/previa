use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use assert_cmd::prelude::*;
use serde_json::json;
use tempfile::TempDir;

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

fn setup_previa_home() -> TempDir {
    TempDir::new().expect("tempdir")
}

fn start_health_server(port: u16) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind health server");
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(_) => break,
            };
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            let request = String::from_utf8_lossy(&buffer);
            let status = if request.starts_with("GET /health ") {
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            } else {
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            };
            let _ = stream.write_all(status.as_bytes());
        }
    })
}

fn write_state_file(
    previa_home: &Path,
    stack: &str,
    main_pid: u32,
    main_port: u16,
    runner_pid: u32,
    runner_port: u16,
) {
    let stack_dir = previa_home.join("stacks").join(stack);
    fs::create_dir_all(stack_dir.join("run")).expect("run dir");
    let runtime_file = stack_dir.join("run/state.json");
    let payload = json!({
        "name": stack,
        "mode": "detached",
        "started_at": "2026-03-11T00:00:00Z",
        "main": {
            "pid": main_pid,
            "address": "127.0.0.1",
            "port": main_port,
            "log_path": stack_dir.join("logs/main.log").display().to_string()
        },
        "runner_port_range": {
            "start": runner_port,
            "end": runner_port
        },
        "attached_runners": [],
        "runners": [
            {
                "pid": runner_pid,
                "address": "127.0.0.1",
                "port": runner_port,
                "log_path": stack_dir.join(format!("logs/runners/{runner_port}.log")).display().to_string()
            }
        ]
    });
    fs::write(runtime_file, serde_json::to_vec_pretty(&payload).expect("encode state"))
        .expect("write state");
}

#[test]
fn status_reports_stopped_when_runtime_is_missing() {
    let temp = setup_previa_home();
    let output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["status", "--name", "missing", "--json"])
        .output()
        .expect("status output");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("status json");
    assert_eq!(json["state"], "stopped");
    assert!(json["main"].is_null());
    assert_eq!(json["runners"].as_array().expect("runners").len(), 0);
}

#[test]
fn status_list_and_ps_report_running_stack_from_runtime_file() {
    let temp = setup_previa_home();
    let stack = "itest";
    let main_port = find_free_port();
    let runner_port = find_free_port();
    let _main_server = start_health_server(main_port);
    let _runner_server = start_health_server(runner_port);
    thread::sleep(Duration::from_millis(100));

    write_state_file(
        temp.path(),
        stack,
        std::process::id(),
        main_port,
        std::process::id(),
        runner_port,
    );

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

    let main_status = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["status", "--name", stack, "--main", "--json"])
        .output()
        .expect("main status");
    assert!(main_status.status.success());
    let main_json: serde_json::Value =
        serde_json::from_slice(&main_status.stdout).expect("main json");
    assert_eq!(main_json["main"]["state"], "running");

    let runner_status = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args([
            "status",
            "--name",
            stack,
            "--runner",
            &format!("127.0.0.1:{runner_port}"),
            "--json",
        ])
        .output()
        .expect("runner status");
    assert!(runner_status.status.success());
    let runner_json: serde_json::Value =
        serde_json::from_slice(&runner_status.stdout).expect("runner json");
    assert_eq!(runner_json["runners"][0]["state"], "running");

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
    assert_eq!(list_json.as_array().expect("list array")[0]["state"], "running");
}

#[test]
fn status_reports_degraded_for_dead_pid_or_unhealthy_probe() {
    let temp = setup_previa_home();
    let stack = "degraded";
    let main_port = find_free_port();
    let _main_server = start_health_server(main_port);
    thread::sleep(Duration::from_millis(100));

    write_state_file(temp.path(), stack, std::process::id(), main_port, 999_999, find_free_port());

    let status_output = cargo_bin()
        .env("PREVIA_HOME", temp.path())
        .args(["status", "--name", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "degraded");
}
