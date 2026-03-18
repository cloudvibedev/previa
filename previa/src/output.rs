use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProcessJson {
    pub role: String,
    pub state: String,
    pub pid: u32,
    pub address: String,
    pub port: u16,
    pub health_url: String,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusProcessJson {
    pub state: String,
    pub pid: u32,
    pub address: String,
    pub port: u16,
    pub health_url: String,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusJson {
    pub name: String,
    pub state: String,
    pub runtime_file: String,
    pub main: Option<StatusProcessJson>,
    pub runners: Vec<StatusProcessJson>,
    pub attached_runners: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListEntryJson {
    pub name: String,
    pub state: String,
    pub runtime_file: String,
}

pub type ProcessView = ProcessJson;

pub fn print_status_human(status: &StatusJson, main_only: bool, runner_only: bool) {
    if main_only {
        if let Some(main) = &status.main {
            println!(
                "{}\t{}\t{}\t{}:{}",
                "main", main.state, main.pid, main.address, main.port
            );
        } else {
            println!("main\tstopped");
        }
        return;
    }

    if runner_only {
        for runner in &status.runners {
            println!(
                "{}\t{}\t{}\t{}:{}",
                "runner", runner.state, runner.pid, runner.address, runner.port
            );
        }
        return;
    }

    println!("{}\t{}", status.name, status.state);
    if let Some(main) = &status.main {
        println!(
            "main\t{}\t{}\t{}:{}",
            main.state, main.pid, main.address, main.port
        );
    }
    for runner in &status.runners {
        println!(
            "runner\t{}\t{}\t{}:{}",
            runner.state, runner.pid, runner.address, runner.port
        );
    }
    if !status.attached_runners.is_empty() {
        println!("attached\t{}", status.attached_runners.join(","));
    }
}

pub fn print_list_human(entries: &[ListEntryJson]) {
    for entry in entries {
        println!("{}\t{}", entry.name, entry.state);
    }
}

pub fn print_process_rows(rows: &[ProcessView]) {
    for row in rows {
        println!(
            "{}\t{}\t{}\t{}:{}\t{}\t{}",
            row.role, row.state, row.pid, row.address, row.port, row.health_url, row.log_path
        );
    }
}
