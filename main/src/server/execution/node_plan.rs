use std::time::Duration;

use reqwest::Client;

use crate::server::models::{NodePlan, RunnerInfo, RunnerRuntimeInfo};

pub async fn collect_active_nodes(client: &Client, runner_endpoints: &[String]) -> Vec<String> {
    collect_runner_statuses(client, runner_endpoints)
        .await
        .into_iter()
        .filter(|runner| runner.active)
        .map(|runner| runner.endpoint)
        .collect()
}

pub async fn collect_runner_statuses(
    client: &Client,
    runner_endpoints: &[String],
) -> Vec<RunnerInfo> {
    let mut runners = Vec::with_capacity(runner_endpoints.len());

    for endpoint in runner_endpoints {
        let (runtime, runtime_error) = fetch_runner_runtime_info(client, endpoint).await;
        runners.push(RunnerInfo {
            endpoint: endpoint.clone(),
            active: is_runner_healthy(client, endpoint).await,
            runtime,
            runtime_error,
        });
    }

    runners
}

pub async fn fetch_runner_runtime_info(
    client: &Client,
    endpoint: &str,
) -> (Option<RunnerRuntimeInfo>, Option<String>) {
    let url = format!("{}/info", endpoint.trim_end_matches('/'));
    match tokio::time::timeout(Duration::from_secs(2), client.get(url).send()).await {
        Ok(Ok(response)) => {
            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return (
                    None,
                    Some(format!("runner /info returned HTTP {}: {}", status, body)),
                );
            }

            match response.json::<RunnerRuntimeInfo>().await {
                Ok(runtime) => (Some(runtime), None),
                Err(err) => (None, Some(format!("invalid /info payload: {}", err))),
            }
        }
        Ok(Err(err)) => (None, Some(format!("runner /info request failed: {}", err))),
        Err(_) => (None, Some("runner /info request timeout".to_owned())),
    }
}

pub async fn is_runner_healthy(client: &Client, endpoint: &str) -> bool {
    let url = format!("{}/health", endpoint.trim_end_matches('/'));

    match tokio::time::timeout(Duration::from_secs(2), client.get(url).send()).await {
        Ok(Ok(response)) => response.status().is_success(),
        _ => false,
    }
}

pub fn calculate_node_plan(
    requested_concurrency: u64,
    rps_per_node: u64,
    nodes_found: usize,
    total_requests: usize,
    concurrency: usize,
) -> NodePlan {
    let required_nodes = requested_concurrency.div_ceil(rps_per_node) as usize;

    let mut nodes_used = required_nodes.min(nodes_found);
    nodes_used = nodes_used.min(total_requests.max(1));
    nodes_used = nodes_used.min(concurrency.max(1));

    if nodes_used == 0 && nodes_found > 0 {
        nodes_used = 1;
    }

    let warning = if required_nodes > nodes_found {
        Some(format!(
            "Requested concurrency {} needs {} nodes at {} req/s capacity per node, but only {} active nodes were found. Distributing across available nodes.",
            requested_concurrency, required_nodes, rps_per_node, nodes_found
        ))
    } else {
        None
    };

    NodePlan {
        requested_nodes: required_nodes,
        nodes_found,
        nodes_used,
        warning,
    }
}

pub fn split_even(total: usize, parts: usize) -> Vec<usize> {
    if parts == 0 {
        return Vec::new();
    }
    let base = total / parts;
    let rem = total % parts;

    (0..parts)
        .map(|i| if i < rem { base + 1 } else { base })
        .collect()
}

pub fn parse_runner_endpoints() -> Vec<String> {
    std::env::var("RUNNER_ENDPOINTS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.trim_end_matches('/').to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::server::execution::node_plan::{calculate_node_plan, split_even};

    #[test]
    fn warns_when_not_enough_nodes_for_requested_rps() {
        let plan = calculate_node_plan(10_000, 1_000, 2, 100_000, 100);
        assert_eq!(plan.requested_nodes, 10);
        assert_eq!(plan.nodes_found, 2);
        assert_eq!(plan.nodes_used, 2);
        assert!(plan.warning.is_some());
    }

    #[test]
    fn does_not_warn_when_capacity_is_enough() {
        let plan = calculate_node_plan(2_000, 1_000, 3, 100_000, 100);
        assert_eq!(plan.requested_nodes, 2);
        assert_eq!(plan.nodes_used, 2);
        assert!(plan.warning.is_none());
    }

    #[test]
    fn splits_evenly() {
        assert_eq!(split_even(10, 3), vec![4, 3, 3]);
    }
}
