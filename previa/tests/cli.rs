use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use assert_cmd::prelude::*;
use tempfile::TempDir;

fn python3_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

fn write_browser_capture_script(path: &Path) {
    let script = r#"#!/bin/sh
printf '%s' "$1" > "$PREVIA_OPEN_CAPTURE"
"#;

    fs::write(path, script).expect("write browser capture script");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn write_fake_binary(path: &Path, label: &str) {
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ] || [ "$1" = "-v" ]; then
  printf '%s 0.0.7\n' "{label}"
  exit 0
fi
exec python3 -u - <<'PY'
import os
import signal
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

address = os.environ.get("ADDRESS", "127.0.0.1")
port = int(os.environ.get("PORT", "0"))
health_status = int(os.environ.get("HEALTH_STATUS", "200"))
health_status_file = os.environ.get("HEALTH_STATUS_FILE")
fail_port = os.environ.get("FAIL_PORT")

if os.environ.get("FAIL_STARTUP") == "1":
    sys.exit(1)
if fail_port and fail_port == str(port):
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
            self.wfile.write(b'{{"pid":1,"memoryBytes":0,"virtualMemoryBytes":0,"cpuUsagePercent":0.0}}')
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, fmt, *args):
        return

httpd = HTTPServer((address, port), Handler)
print("fake binary service listening on {{}}:{{}} pid={{}}".format(address, port, os.getpid()), flush=True)

def stop(_signum, _frame):
    httpd.shutdown()

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
httpd.serve_forever()
PY
"#
    );

    fs::write(path, script).expect("write fake binary");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn write_fake_docker(path: &Path) {
    let script = r#"#!/bin/sh
exec python3 -u - "$@" <<'PY'
import json
import os
import pathlib
import signal
import subprocess
import sys
import time

SERVER_CODE = r"""
import os
import signal
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

address = os.environ.get("ADDRESS", "127.0.0.1")
port = int(os.environ.get("PORT", "0"))
health_status = int(os.environ.get("HEALTH_STATUS", "200"))
health_status_file = os.environ.get("HEALTH_STATUS_FILE")
fail_port = os.environ.get("FAIL_PORT")

if os.environ.get("FAIL_STARTUP") == "1":
    sys.exit(1)
if fail_port and fail_port == str(port):
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
print(f"fake compose service listening on {address}:{port} pid={os.getpid()}", flush=True)

def stop(_signum, _frame):
    httpd.shutdown()

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
httpd.serve_forever()
"""

STATE_PATH = pathlib.Path(
    os.environ.get(
        "PREVIA_FAKE_DOCKER_STATE",
        str(pathlib.Path(os.environ["PREVIA_HOME"]) / "fake-docker-state.json"),
    )
)
LOG_ROOT = pathlib.Path(os.environ["PREVIA_HOME"]) / "fake-docker-logs"


def load_state():
    if not STATE_PATH.exists():
        return {"projects": {}}
    return json.loads(STATE_PATH.read_text(encoding="utf-8"))


def save_state(state):
    STATE_PATH.parent.mkdir(parents=True, exist_ok=True)
    STATE_PATH.write_text(json.dumps(state, indent=2), encoding="utf-8")


def append_log():
    log_path = os.environ.get("PREVIA_DOCKER_LOG")
    if not log_path:
        return
    with open(log_path, "a", encoding="utf-8") as fh:
        fh.write(" ".join(sys.argv[1:]) + "\n")


def process_exists(pid):
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def stop_pid(pid):
    if not process_exists(pid):
        return
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return
    for _ in range(20):
        if not process_exists(pid):
            return
        time.sleep(0.05)
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError:
        return


def project_entry(state, project):
    return state.setdefault("projects", {}).setdefault(project, {"services": {}})


def spawn_service(project, service_name, service):
    service_log_dir = LOG_ROOT / project
    service_log_dir.mkdir(parents=True, exist_ok=True)
    log_path = service_log_dir / f"{service_name}.log"
    log_handle = open(log_path, "w", encoding="utf-8")

    env = os.environ.copy()
    for key, value in service.get("environment", {}).items():
        env[key] = str(value)

    ports = service.get("ports", [])
    bind_address = "127.0.0.1"
    bind_port = 0
    if ports:
        bind_address = str(ports[0].get("host_ip", "127.0.0.1"))
        bind_port = int(ports[0].get("published", 0))

    env["ADDRESS"] = bind_address
    env["PORT"] = str(bind_port)
    process = subprocess.Popen(
        ["python3", "-u", "-c", SERVER_CODE],
        env=env,
        stdout=log_handle,
        stderr=log_handle,
        close_fds=True,
    )
    time.sleep(0.2)
    if process.poll() is not None:
        log_handle.close()
        return None

    log_handle.close()
    return {
        "container_id": f"{project}_{service_name}",
        "service_name": service_name,
        "pid": process.pid,
        "running": True,
        "log_path": str(log_path),
    }


def stop_service(metadata):
    if metadata.get("running") and metadata.get("pid"):
        stop_pid(int(metadata["pid"]))
    metadata["running"] = False
    metadata["pid"] = 0


def render_logs(service_names, project_state, tail):
    chunks = []
    for service_name in service_names:
        metadata = project_state["services"].get(service_name)
        if not metadata:
            continue
        path = pathlib.Path(metadata["log_path"])
        if not path.exists():
            continue
        contents = path.read_text(encoding="utf-8")
        if tail is not None:
            lines = contents.splitlines()
            if len(lines) > tail:
                lines = lines[-tail:]
            contents = "\n".join(lines)
            if lines:
                contents += "\n"
        chunks.append(contents)
    return "".join(chunks)


append_log()
argv = sys.argv[1:]
if not argv:
    sys.exit(1)

if argv[0] == "pull":
    sys.exit(0)

if argv[0] == "inspect":
    state = load_state()
    records = []
    for container_id in argv[1:]:
        for project in state.get("projects", {}).values():
            for metadata in project.get("services", {}).values():
                if metadata["container_id"] == container_id:
                    records.append(
                        {
                            "LogPath": metadata["log_path"],
                            "State": {
                                "Running": metadata["running"],
                                "Pid": metadata["pid"],
                            },
                        }
                    )
    print(json.dumps(records))
    sys.exit(0)

if argv[0] != "compose":
    sys.exit(1)

idx = 1
project = None
compose_file = None
while idx < len(argv):
    if argv[idx] == "-p":
        project = argv[idx + 1]
        idx += 2
    elif argv[idx] == "-f":
        compose_file = argv[idx + 1]
        idx += 2
    else:
        break

command = argv[idx]
rest = argv[idx + 1 :]
state = load_state()
project_state = project_entry(state, project)

if command == "up":
    detached = "-d" in rest
    force_recreate = "--force-recreate" in rest
    requested_services = [value for value in rest if not value.startswith("-")]
    doc = json.loads(pathlib.Path(compose_file).read_text(encoding="utf-8"))
    services = doc.get("services", {})
    if requested_services:
        services = {name: services[name] for name in requested_services}

    if force_recreate:
        for metadata in project_state["services"].values():
            stop_service(metadata)
        project_state["services"] = {}

    started = []
    for service_name, service in services.items():
        metadata = spawn_service(project, service_name, service)
        if metadata is None:
            for started_service in started:
                stop_service(project_state["services"][started_service])
                del project_state["services"][started_service]
            save_state(state)
            sys.exit(1)
        project_state["services"][service_name] = metadata
        started.append(service_name)

    save_state(state)
    if detached:
        sys.exit(0)

    try:
        while True:
            time.sleep(0.25)
    except KeyboardInterrupt:
        for metadata in project_state["services"].values():
            stop_service(metadata)
        save_state(state)
        sys.exit(0)

elif command == "down":
    for metadata in project_state["services"].values():
        stop_service(metadata)
    state.get("projects", {}).pop(project, None)
    save_state(state)
    sys.exit(0)

elif command == "stop":
    for service_name in rest:
        metadata = project_state["services"].get(service_name)
        if metadata:
            stop_service(metadata)
    save_state(state)
    sys.exit(0)

elif command == "rm":
    service_names = [value for value in rest if not value.startswith("-")]
    for service_name in service_names:
        metadata = project_state["services"].get(service_name)
        if metadata:
            stop_service(metadata)
            del project_state["services"][service_name]
    save_state(state)
    sys.exit(0)

elif command == "ps":
    service_names = [value for value in rest if not value.startswith("-")]
    if not service_names:
        service_names = sorted(project_state["services"].keys())
    for service_name in service_names:
        metadata = project_state["services"].get(service_name)
        if metadata:
            print(metadata["container_id"])
    sys.exit(0)

elif command == "logs":
    tail = None
    follow = False
    service_names = []
    idx = 0
    while idx < len(rest):
        value = rest[idx]
        if value == "--tail":
            tail = int(rest[idx + 1])
            idx += 2
        elif value == "--follow":
            follow = True
            idx += 1
        elif value == "--no-color":
            idx += 1
        else:
            service_names.append(value)
            idx += 1
    if not service_names:
        service_names = [name for name in sorted(project_state["services"].keys())]
    sys.stdout.write(render_logs(service_names, project_state, tail))
    sys.stdout.flush()
    if follow:
        sys.exit(0)
    sys.exit(0)

sys.exit(1)
PY
"#;

    fs::write(path, script).expect("write fake docker script");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn cargo_bin() -> Command {
    Command::cargo_bin("previa").expect("previa binary")
}

fn prepend_path(dir: &Path) -> OsString {
    let mut value = OsString::from(dir.as_os_str());
    if let Some(current) = std::env::var_os("PATH") {
        value.push(":");
        value.push(current);
    }
    value
}

fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind free port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn setup_fake_docker() -> TempDir {
    let temp = TempDir::new().expect("tempdir");
    let docker_dir = temp.path().join("docker-bin");
    fs::create_dir_all(&docker_dir).expect("docker dir");
    write_fake_docker(&docker_dir.join("docker"));
    temp
}

fn setup_fake_binaries(temp: &TempDir) {
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    write_fake_binary(&bin_dir.join("previa-main"), "previa-main");
    write_fake_binary(&bin_dir.join("previa-runner"), "previa-runner");
}

fn docker_env(temp: &TempDir, command: &mut Command) {
    command
        .env("PREVIA_HOME", temp.path())
        .env("PATH", prepend_path(&temp.path().join("docker-bin")));
}

#[test]
fn dry_run_rejects_detach() {
    let temp = setup_fake_docker();
    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    command
        .args(["up", "--dry-run", "--detach"])
        .assert()
        .failure();
}

#[test]
fn up_bin_rejects_version_override() {
    let temp = setup_fake_docker();
    setup_fake_binaries(&temp);

    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    let output = command
        .args(["up", "--bin", "--version", "0.0.7"])
        .output()
        .expect("up output");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("--version cannot be used with --bin"));
}

#[test]
fn up_bin_fails_when_local_binaries_are_missing() {
    let temp = setup_fake_docker();

    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    let output = command
        .current_dir(temp.path())
        .args(["up", "--bin"])
        .output()
        .expect("up output");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("missing binary 'previa-main'"));
}

#[test]
fn pull_defaults_to_all_latest_without_local_binaries() {
    let temp = setup_fake_docker();
    let docker_log = temp.path().join("docker.log");

    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    command
        .env("PREVIA_DOCKER_LOG", &docker_log)
        .args(["pull"])
        .assert()
        .success();

    let output = fs::read_to_string(&docker_log).expect("docker log");
    assert!(output.contains("pull ghcr.io/cloudvibedev/main:latest"));
    assert!(output.contains("pull ghcr.io/cloudvibedev/runner:latest"));
}

#[test]
fn pull_accepts_explicit_version_for_single_target() {
    let temp = setup_fake_docker();
    let docker_log = temp.path().join("docker.log");

    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    command
        .env("PREVIA_DOCKER_LOG", &docker_log)
        .args(["pull", "runner", "--version", "0.0.7"])
        .assert()
        .success();

    let output = fs::read_to_string(&docker_log).expect("docker log");
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        vec!["pull ghcr.io/cloudvibedev/runner:0.0.7"]
    );
}

#[test]
fn dry_run_resolves_compose_without_writing_runtime() {
    let temp = setup_fake_docker();
    let compose = temp.path().join("previa-compose.yaml");
    let main_port = find_free_port();
    let runner_port = find_free_port();
    fs::write(
        &compose,
        format!(
            r#"version: 1
main:
  address: 127.0.0.1
  port: {main_port}
runners:
  local:
    address: 127.0.0.1
    count: 1
    port_range:
      start: {runner_port}
      end: {runner_port}
"#
        ),
    )
    .expect("write compose");

    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    command
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

    let temp = setup_fake_docker();
    let stack = "itest";
    let main_port = find_free_port();
    let runner_port = find_free_port();

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    up.args([
        "up",
        "--context",
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

    let mut status = cargo_bin();
    docker_env(&temp, &mut status);
    let status_output = status
        .args(["status", "--context", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "running");
    assert_eq!(status_json["main"]["address"], "127.0.0.1");
    assert_eq!(status_json["runners"][0]["port"], runner_port);

    let mut ps = cargo_bin();
    docker_env(&temp, &mut ps);
    let ps_output = ps
        .args(["ps", "--context", stack, "--json"])
        .output()
        .expect("ps output");
    assert!(ps_output.status.success());
    let ps_json: serde_json::Value = serde_json::from_slice(&ps_output.stdout).expect("ps json");
    assert_eq!(ps_json.as_array().expect("ps array").len(), 2);
    assert_eq!(ps_json[0]["role"], "main");
    assert_eq!(ps_json[1]["role"], "runner");

    let mut list = cargo_bin();
    docker_env(&temp, &mut list);
    let list_output = list.args(["list", "--json"]).output().expect("list output");
    assert!(list_output.status.success());
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("list json");
    assert_eq!(list_json.as_array().expect("list array")[0]["name"], stack);

    let mut logs = cargo_bin();
    docker_env(&temp, &mut logs);
    let logs_output = logs
        .args(["logs", "--context", stack, "--main"])
        .output()
        .expect("logs output");
    assert!(logs_output.status.success());
    let logs = String::from_utf8(logs_output.stdout).expect("utf8 logs");
    assert!(logs.contains("fake compose service listening"));

    let mut down = cargo_bin();
    docker_env(&temp, &mut down);
    down.args(["down", "--context", stack]).assert().success();

    assert!(
        !temp
            .path()
            .join("stacks")
            .join(stack)
            .join("run/state.json")
            .exists()
    );
}

#[test]
fn detached_binary_lifecycle_supports_status_ps_logs_restart_and_down() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    setup_fake_binaries(&temp);
    let stack = "bin-itest";
    let main_port = find_free_port();
    let runner_port = find_free_port();

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    up.args([
        "up",
        "--bin",
        "--context",
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

    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(
            temp.path()
                .join("stacks")
                .join(stack)
                .join("run/state.json"),
        )
        .expect("runtime state"),
    )
    .expect("runtime json");
    assert_eq!(state["backend"], "bin");
    assert!(state["main"]["pid"].as_u64().unwrap_or_default() > 0);

    let mut status = cargo_bin();
    docker_env(&temp, &mut status);
    let status_output = status
        .args(["status", "--context", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "running");
    assert_eq!(status_json["main"]["address"], "127.0.0.1");

    let mut ps = cargo_bin();
    docker_env(&temp, &mut ps);
    let ps_output = ps
        .args(["ps", "--context", stack, "--json"])
        .output()
        .expect("ps output");
    assert!(ps_output.status.success());
    let ps_json: serde_json::Value = serde_json::from_slice(&ps_output.stdout).expect("ps json");
    assert_eq!(ps_json.as_array().expect("ps array").len(), 2);
    assert_eq!(ps_json[0]["role"], "main");
    assert!(ps_json[0]["pid"].as_u64().unwrap_or_default() > 0);

    let mut logs = cargo_bin();
    docker_env(&temp, &mut logs);
    let logs_output = logs
        .args(["logs", "--context", stack, "--main"])
        .output()
        .expect("logs output");
    assert!(logs_output.status.success());
    let logs = String::from_utf8(logs_output.stdout).expect("utf8 logs");
    assert!(logs.contains("fake binary service listening"));

    let mut restart = cargo_bin();
    docker_env(&temp, &mut restart);
    restart
        .args(["restart", "--context", stack])
        .assert()
        .success();

    let mut restart_with_version = cargo_bin();
    docker_env(&temp, &mut restart_with_version);
    let restart_output = restart_with_version
        .args(["restart", "--context", stack, "--version", "0.0.8"])
        .output()
        .expect("restart output");
    assert!(!restart_output.status.success());
    let restart_stderr = String::from_utf8(restart_output.stderr).expect("utf8 stderr");
    assert!(restart_stderr.contains("--version is only supported for compose-backed runtimes"));

    let mut down = cargo_bin();
    docker_env(&temp, &mut down);
    down.args(["down", "--context", stack]).assert().success();

    assert!(
        !temp
            .path()
            .join("stacks")
            .join(stack)
            .join("run/state.json")
            .exists()
    );
}

#[test]
fn down_runner_removes_selected_runner_and_rewrites_runtime() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let stack = "partial";
    let main_port = find_free_port();
    let runner_start = find_free_port();
    let runner_end = runner_start + 1;

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    up.args([
        "up",
        "--context",
        stack,
        "--detach",
        "--main-address",
        "127.0.0.1",
        "-p",
        &main_port.to_string(),
        "--runner-address",
        "127.0.0.1",
        "-P",
        &format!("{runner_start}:{runner_end}"),
        "-r",
        "2",
    ])
    .assert()
    .success();

    let mut down = cargo_bin();
    docker_env(&temp, &mut down);
    down.args([
        "down",
        "--context",
        stack,
        "--runner",
        &runner_start.to_string(),
    ])
    .assert()
    .success();

    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(
            temp.path()
                .join("stacks")
                .join(stack)
                .join("run/state.json"),
        )
        .expect("runtime state"),
    )
    .expect("runtime json");
    assert_eq!(state["runners"].as_array().expect("runner array").len(), 1);
    assert_eq!(state["runners"][0]["port"], runner_end);

    let compose_file = fs::read_to_string(
        temp.path()
            .join("stacks")
            .join(stack)
            .join("run/docker-compose.generated.yaml"),
    )
    .expect("compose file");
    assert!(!compose_file.contains(&format!("runner-{runner_start}")));
    assert!(compose_file.contains(&format!("runner-{runner_end}")));
}

#[test]
fn restart_allows_overriding_image_tag() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let stack = "retag";
    let main_port = find_free_port();
    let runner_port = find_free_port();

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    up.args([
        "up",
        "--context",
        stack,
        "--detach",
        "--version",
        "0.0.7",
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

    let mut restart = cargo_bin();
    docker_env(&temp, &mut restart);
    restart
        .args(["restart", "--context", stack, "--version", "0.0.8"])
        .assert()
        .success();

    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(
            temp.path()
                .join("stacks")
                .join(stack)
                .join("run/state.json"),
        )
        .expect("runtime state"),
    )
    .expect("runtime json");
    assert_eq!(state["image_tag"], "0.0.8");

    let compose_file = fs::read_to_string(
        temp.path()
            .join("stacks")
            .join(stack)
            .join("run/docker-compose.generated.yaml"),
    )
    .expect("compose file");
    assert!(compose_file.contains("ghcr.io/cloudvibedev/main:0.0.8"));
}

#[test]
fn up_fails_early_when_context_is_already_running() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let stack = "busy";
    let main_port = find_free_port();
    let runner_port = find_free_port();

    let mut first = cargo_bin();
    docker_env(&temp, &mut first);
    first
        .args([
            "up",
            "--context",
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

    let next_main_port = find_free_port();
    let mut second = cargo_bin();
    docker_env(&temp, &mut second);
    let output = second
        .args([
            "up",
            "--context",
            stack,
            "--main-port",
            &next_main_port.to_string(),
        ])
        .output()
        .expect("up output");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(!stderr.trim().is_empty());
}

#[test]
fn down_all_context_stops_every_detached_context() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let alpha_main_port = find_free_port();
    let alpha_runner_port = find_free_port();
    let beta_main_port = find_free_port();
    let beta_runner_port = find_free_port();

    let mut alpha = cargo_bin();
    docker_env(&temp, &mut alpha);
    alpha
        .args([
            "up",
            "--context",
            "alpha",
            "--detach",
            "--main-address",
            "127.0.0.1",
            "-p",
            &alpha_main_port.to_string(),
            "--runner-address",
            "127.0.0.1",
            "-P",
            &format!("{alpha_runner_port}:{alpha_runner_port}"),
            "-r",
            "1",
        ])
        .assert()
        .success();

    let mut beta = cargo_bin();
    docker_env(&temp, &mut beta);
    beta.args([
        "up",
        "--context",
        "beta",
        "--detach",
        "--main-address",
        "127.0.0.1",
        "-p",
        &beta_main_port.to_string(),
        "--runner-address",
        "127.0.0.1",
        "-P",
        &format!("{beta_runner_port}:{beta_runner_port}"),
        "-r",
        "1",
    ])
    .assert()
    .success();

    let mut down = cargo_bin();
    docker_env(&temp, &mut down);
    down.args(["down", "--all-contexts"]).assert().success();

    assert!(!temp.path().join("stacks/alpha/run/state.json").exists());
    assert!(!temp.path().join("stacks/beta/run/state.json").exists());
}

#[test]
fn logs_supports_tail_count() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let stack = "tailtest";
    let main_port = find_free_port();
    let runner_port = find_free_port();

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    up.args([
        "up",
        "--context",
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

    let main_log = temp
        .path()
        .join("fake-docker-logs")
        .join("previa_tailtest")
        .join("main.log");
    fs::OpenOptions::new()
        .append(true)
        .open(&main_log)
        .expect("open main log")
        .write_all(b"line-one\nline-two\nline-three\n")
        .expect("append main log");

    let mut logs = cargo_bin();
    docker_env(&temp, &mut logs);
    let logs_output = logs
        .args(["logs", "--context", stack, "--main", "-t", "2"])
        .output()
        .expect("logs output");
    assert!(logs_output.status.success());
    let logs = String::from_utf8(logs_output.stdout).expect("utf8 logs");
    assert_eq!(logs, "line-two\nline-three\n");
}

#[test]
fn up_prompts_and_accepts_shifted_main_port_on_enter() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let occupied_main_port = find_free_port();
    let runner_port = find_free_port();
    let _occupied = TcpListener::bind(("127.0.0.1", occupied_main_port)).expect("occupy main");

    let output = run_command_with_stdin(
        temp.path(),
        [
            "up",
            "--detach",
            "--main-address",
            "127.0.0.1",
            "-p",
            &occupied_main_port.to_string(),
            "--runner-address",
            "127.0.0.1",
            "-P",
            &format!("{runner_port}:{runner_port}"),
        ],
        "\n",
    );
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("press [Y] to continue with main port"));

    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("stacks/default/run/state.json")).expect("runtime state"),
    )
    .expect("runtime json");
    assert_eq!(state["main"]["port"], occupied_main_port + 100);
}

#[test]
fn up_prompts_and_accepts_shifted_runner_range_on_enter() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let main_port = find_free_port();
    let occupied = TcpListener::bind("127.0.0.1:0").expect("occupy runner port");
    let occupied_runner_port = occupied.local_addr().expect("occupied runner addr").port();

    let output = run_command_with_stdin(
        temp.path(),
        [
            "up",
            "--detach",
            "--main-address",
            "127.0.0.1",
            "-p",
            &main_port.to_string(),
            "--runner-address",
            "127.0.0.1",
            "-P",
            &format!("{occupied_runner_port}:{occupied_runner_port}"),
        ],
        "\n",
    );
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("press [Y] to continue with runner ports starting at"));

    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("stacks/default/run/state.json")).expect("runtime state"),
    )
    .expect("runtime json");
    assert_eq!(
        state["runner_port_range"]["start"],
        occupied_runner_port + 100
    );
}

#[test]
fn status_reports_degraded_when_health_is_not_200() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
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

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    up.args([
        "up",
        "--context",
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

    fs::write(&health_status_file, "204\n").expect("health status file");

    let mut status = cargo_bin();
    docker_env(&temp, &mut status);
    let status_output = status
        .args(["status", "--context", stack, "--json"])
        .output()
        .expect("status output");
    assert!(status_output.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("status json");
    assert_eq!(status_json["state"], "degraded");
    assert_eq!(status_json["main"]["state"], "degraded");
    assert_eq!(status_json["runners"][0]["state"], "running");
}

#[test]
fn up_rejects_zero_ports_from_cli_and_compose() {
    let temp = setup_fake_docker();

    let mut main_port = cargo_bin();
    docker_env(&temp, &mut main_port);
    main_port
        .args(["up", "--dry-run", "--main-port", "0"])
        .assert()
        .failure();

    let mut runner_port = cargo_bin();
    docker_env(&temp, &mut runner_port);
    runner_port
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

    let mut main_compose = cargo_bin();
    docker_env(&temp, &mut main_compose);
    main_compose
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

    let mut runner_compose = cargo_bin();
    docker_env(&temp, &mut runner_compose);
    runner_compose
        .args([
            "up",
            "--dry-run",
            runner_port_zero.to_str().expect("compose path"),
        ])
        .assert()
        .failure();
}

#[test]
fn up_leaves_no_runtime_state_when_compose_startup_fails() {
    if !python3_available() {
        return;
    }

    let temp = setup_fake_docker();
    let stack = "rollback";
    let main_port = find_free_port();
    let runner_port = find_free_port();
    let failing_runner_port = runner_port + 1;
    let stack_config_dir = temp.path().join("stacks").join(stack).join("config");
    fs::create_dir_all(&stack_config_dir).expect("stack config dir");
    fs::write(
        stack_config_dir.join("runner.env"),
        format!("FAIL_PORT={failing_runner_port}\n"),
    )
    .expect("runner env");

    let mut up = cargo_bin();
    docker_env(&temp, &mut up);
    let output = up
        .args([
            "up",
            "--context",
            stack,
            "--detach",
            "--main-address",
            "127.0.0.1",
            "-p",
            &main_port.to_string(),
            "--runner-address",
            "127.0.0.1",
            "-P",
            &format!("{runner_port}:{failing_runner_port}"),
            "-r",
            "2",
        ])
        .output()
        .expect("up output");

    assert!(!output.status.success());
    assert!(
        !temp
            .path()
            .join("stacks")
            .join(stack)
            .join("run/state.json")
            .exists()
    );
}

#[test]
fn open_launches_app_with_encoded_main_context_url() {
    let temp = setup_fake_docker();
    let stack = "other";
    let stack_dir = temp.path().join("stacks").join(stack);
    let run_dir = stack_dir.join("run");
    fs::create_dir_all(&run_dir).expect("run dir");
    let browser = temp.path().join("capture-browser.sh");
    let capture = temp.path().join("opened-url.txt");
    write_browser_capture_script(&browser);

    fs::write(
        run_dir.join("state.json"),
        format!(
            r#"{{
  "name": "{stack}",
  "mode": "detached",
  "started_at": "2026-03-11T00:00:00Z",
  "image_tag": "latest",
  "compose_file": "{}",
  "compose_project": "previa_{stack}",
  "main": {{
    "service_name": "main",
    "address": "0.0.0.0",
    "port": 5588
  }},
  "runner_port_range": {{
    "start": 55880,
    "end": 55979
  }},
  "attached_runners": [],
  "runners": []
}}"#,
            stack_dir
                .join("run")
                .join("docker-compose.generated.yaml")
                .display()
        ),
    )
    .expect("runtime state");

    let mut command = cargo_bin();
    docker_env(&temp, &mut command);
    let output = command
        .env("PREVIA_OPEN_BROWSER", &browser)
        .env("PREVIA_OPEN_CAPTURE", &capture)
        .args(["open", "--context", stack])
        .output()
        .expect("open output");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let opened = fs::read_to_string(&capture).expect("captured URL");
    let expected = "https://app.previa.dev?add_context=http%3A%2F%2F127.0.0.1%3A5588";
    assert_eq!(opened, expected);
    assert_eq!(stdout.trim(), expected);
}

fn run_command_with_stdin<const N: usize>(
    previa_home: &Path,
    args: [&str; N],
    stdin_input: &str,
) -> std::process::Output {
    let mut command = cargo_bin();
    command
        .env("PREVIA_HOME", previa_home)
        .env("PATH", prepend_path(&previa_home.join("docker-bin")))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args);

    let mut child = command.spawn().expect("spawn command");
    child
        .stdin
        .as_mut()
        .expect("stdin pipe")
        .write_all(stdin_input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait with output")
}
