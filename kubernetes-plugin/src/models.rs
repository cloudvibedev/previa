use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationCreateRequest {
    pub execution_id: String,
    pub pipeline_id: String,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationStatus {
    pub reservation_id: String,
    pub status: String,
    pub requested_runners: usize,
    pub ready_runners: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservation_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub runners: Vec<ReservationRunner>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationRunner {
    pub id: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct KarpenterProvisionerConfig {
    pub kind: String,
    pub provider: String,
    pub resource_mode: KarpenterResourceMode,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum KarpenterResourceMode {
    Managed,
    Reference,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AwsNodeProfile {
    pub node_pool: String,
    pub ec2_node_class: String,
    pub instance_families: Vec<String>,
    pub instance_sizes: Vec<String>,
    pub expire_after: String,
    pub consolidate_after: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}
