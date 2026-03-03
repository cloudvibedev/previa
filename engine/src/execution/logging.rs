use tracing::debug;

use crate::core::types::{StepRequest, StepResponse};

pub(crate) fn log_step_request(step_id: &str, request: &StepRequest) {
    debug!(
        "{}",
        serde_json::json!({
            "type": "request",
            "stepId": step_id,
            "method": request.method,
            "path": request.url,
            "headers": request.headers,
            "body": request.body
        })
    );
}

pub(crate) fn log_step_response(
    step_id: &str,
    response: Option<&StepResponse>,
    error: Option<&str>,
) {
    match response {
        Some(response) => {
            debug!(
                "{}",
                serde_json::json!({
                    "type": "response",
                    "stepId": step_id,
                    "status_code": response.status,
                    "headers": response.headers,
                    "body": response.body,
                    "error": error
                })
            );
        }
        None => {
            debug!(
                "{}",
                serde_json::json!({
                    "type": "response",
                    "stepId": step_id,
                    "status_code": serde_json::Value::Null,
                    "headers": serde_json::Value::Null,
                    "body": serde_json::Value::Null,
                    "error": error
                })
            );
        }
    }
}
