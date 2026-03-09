use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tracing::info;

use crate::server::db::{
    list_project_records, list_project_spec_records, load_pipelines_for_project,
    load_project_record, project_exists,
};
use crate::server::docs::build_openapi_document;
use crate::server::execution::collect_runner_statuses;
use crate::server::mcp::models::{
    InitializeParams, ListProjectsToolArgs, McpPeerInfo, McpRequest, McpResponse, McpSession,
    ProjectByIdArgs, SUPPORTED_PROTOCOL_VERSIONS, ToolCallParams, ToolCallResult, ToolDefinition,
    ToolTextContent, ToolsListParams, ValidateOpenApiToolArgs,
};
use crate::server::models::{OrchestratorInfoResponse, ProjectListQuery};
use crate::server::state::AppState;
use crate::server::utils::new_uuid_v7;
use crate::server::validation::openapi::validate_openapi_source;

const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;
const INVALID_SESSION: i32 = -32001;

pub enum McpHttpOutcome {
    Response {
        response: McpResponse,
        session_id: Option<String>,
        protocol_version: Option<String>,
    },
    Accepted,
}

pub async fn process_request(
    state: &AppState,
    session_id: Option<&str>,
    protocol_version_header: Option<&str>,
    request: McpRequest,
) -> McpHttpOutcome {
    if request.jsonrpc != crate::server::mcp::models::JSON_RPC_VERSION {
        return McpHttpOutcome::Response {
            response: McpResponse::error(request.id, INVALID_REQUEST, "jsonrpc must be 2.0"),
            session_id: None,
            protocol_version: None,
        };
    }

    let Some(request_id) = request.id.clone() else {
        if request.method == "notifications/initialized" {
            return McpHttpOutcome::Accepted;
        }

        return McpHttpOutcome::Response {
            response: McpResponse::error(None, INVALID_REQUEST, "request id is required"),
            session_id: None,
            protocol_version: None,
        };
    };

    match request.method.as_str() {
        "initialize" => handle_initialize(state, request_id, request.params).await,
        "ping" => McpHttpOutcome::Response {
            response: McpResponse::success(request_id, json!({})),
            session_id: session_id.map(str::to_owned),
            protocol_version: None,
        },
        "tools/list" => {
            let session = match require_session(state, session_id, protocol_version_header).await {
                Ok(session) => session,
                Err(response) => {
                    return McpHttpOutcome::Response {
                        response,
                        session_id: None,
                        protocol_version: None,
                    };
                }
            };
            let params = match parse_optional_params::<ToolsListParams>(request.params) {
                Ok(params) => params,
                Err(response) => {
                    return McpHttpOutcome::Response {
                        response: McpResponse::error(Some(request_id), INVALID_PARAMS, response),
                        session_id: session_id.map(str::to_owned),
                        protocol_version: Some(session.protocol_version),
                    };
                }
            };
            let _ = params.meta.as_ref();
            if params.cursor.is_some() {
                return McpHttpOutcome::Response {
                    response: McpResponse::error(
                        Some(request_id),
                        INVALID_PARAMS,
                        "cursor pagination is not supported",
                    ),
                    session_id: session_id.map(str::to_owned),
                    protocol_version: Some(session.protocol_version),
                };
            }

            McpHttpOutcome::Response {
                response: McpResponse::success(request_id, json!({ "tools": tool_definitions() })),
                session_id: session_id.map(str::to_owned),
                protocol_version: Some(session.protocol_version),
            }
        }
        "tools/call" => {
            let session = match require_session(state, session_id, protocol_version_header).await {
                Ok(session) => session,
                Err(response) => {
                    return McpHttpOutcome::Response {
                        response,
                        session_id: None,
                        protocol_version: None,
                    };
                }
            };
            let params = match parse_params::<ToolCallParams>(request.params) {
                Ok(params) => params,
                Err(message) => {
                    return McpHttpOutcome::Response {
                        response: McpResponse::error(Some(request_id), INVALID_PARAMS, message),
                        session_id: session_id.map(str::to_owned),
                        protocol_version: Some(session.protocol_version),
                    };
                }
            };
            let _ = params.meta.as_ref();

            match execute_tool(state, params).await {
                Ok(result) => McpHttpOutcome::Response {
                    response: McpResponse::success(
                        request_id,
                        serde_json::to_value(result).unwrap(),
                    ),
                    session_id: session_id.map(str::to_owned),
                    protocol_version: Some(session.protocol_version),
                },
                Err(response) => McpHttpOutcome::Response {
                    response: McpResponse::error(Some(request_id), INTERNAL_ERROR, response),
                    session_id: session_id.map(str::to_owned),
                    protocol_version: Some(session.protocol_version),
                },
            }
        }
        _ => McpHttpOutcome::Response {
            response: McpResponse::error(Some(request_id), METHOD_NOT_FOUND, "method not found"),
            session_id: session_id.map(str::to_owned),
            protocol_version: None,
        },
    }
}

pub async fn delete_session(state: &AppState, session_id: Option<&str>) -> bool {
    let Some(session_id) = session_id else {
        return false;
    };
    state
        .mcp_sessions
        .write()
        .await
        .remove(session_id)
        .is_some()
}

async fn handle_initialize(
    state: &AppState,
    request_id: Value,
    params: Option<Value>,
) -> McpHttpOutcome {
    let params = match parse_params::<InitializeParams>(params) {
        Ok(params) => params,
        Err(message) => {
            return McpHttpOutcome::Response {
                response: McpResponse::error(Some(request_id), INVALID_PARAMS, message),
                session_id: None,
                protocol_version: None,
            };
        }
    };

    if !SUPPORTED_PROTOCOL_VERSIONS.contains(&params.protocol_version.as_str()) {
        return McpHttpOutcome::Response {
            response: McpResponse::error(
                Some(request_id),
                INVALID_PARAMS,
                format!(
                    "unsupported protocolVersion '{}'; supported versions: {}",
                    params.protocol_version,
                    SUPPORTED_PROTOCOL_VERSIONS.join(", ")
                ),
            ),
            session_id: None,
            protocol_version: None,
        };
    }

    if let Some(client_info) = params.client_info.as_ref() {
        info!(
            client_name = client_info.name,
            client_version = client_info.version,
            protocol_version = params.protocol_version,
            "mcp client initialized"
        );
    }
    let _ = params.meta.as_ref();
    if !params.capabilities.is_null() {
        info!(capabilities = %params.capabilities, "mcp client capabilities received");
    }

    let session_id = new_uuid_v7();
    state.mcp_sessions.write().await.insert(
        session_id.clone(),
        McpSession {
            protocol_version: params.protocol_version.clone(),
        },
    );

    McpHttpOutcome::Response {
        response: McpResponse::success(
            request_id,
            json!({
                "protocolVersion": params.protocol_version,
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": McpPeerInfo {
                    name: env!("CARGO_PKG_NAME").to_owned(),
                    title: Some("Previa Main MCP".to_owned()),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                "instructions": "Use the available tools to inspect orchestrator health, projects, pipelines, OpenAPI specs, and to validate OpenAPI source content."
            }),
        ),
        session_id: Some(session_id),
        protocol_version: Some(params.protocol_version),
    }
}

async fn require_session(
    state: &AppState,
    session_id: Option<&str>,
    protocol_version_header: Option<&str>,
) -> Result<McpSession, McpResponse> {
    let Some(session_id) = session_id else {
        return Err(McpResponse::error(
            None,
            INVALID_SESSION,
            "missing MCP-Session-Id header",
        ));
    };

    let Some(session) = state.mcp_sessions.read().await.get(session_id).cloned() else {
        return Err(McpResponse::error(
            None,
            INVALID_SESSION,
            "unknown MCP session",
        ));
    };

    if let Some(protocol_version) = protocol_version_header {
        if protocol_version != session.protocol_version {
            return Err(McpResponse::error(
                None,
                INVALID_REQUEST,
                format!(
                    "MCP-Protocol-Version header '{}' does not match negotiated session version '{}'",
                    protocol_version, session.protocol_version
                ),
            ));
        }
    }

    Ok(session)
}

async fn execute_tool(state: &AppState, params: ToolCallParams) -> Result<ToolCallResult, String> {
    match params.name.as_str() {
        "health" => Ok(tool_success(json!({ "status": "ok" }))),
        "get_info" => {
            let runners = collect_runner_statuses(&state.client, &state.runner_endpoints).await;
            let payload = OrchestratorInfoResponse {
                total_runners: runners.len(),
                active_runners: runners.iter().filter(|runner| runner.active).count(),
                runners,
            };
            Ok(tool_success(serde_json::to_value(payload).unwrap()))
        }
        "get_openapi_document" => Ok(tool_success(
            serde_json::to_value(build_openapi_document()).unwrap(),
        )),
        "list_projects" => {
            let args = parse_tool_arguments::<ListProjectsToolArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            let projects = list_project_records(
                &state.db,
                ProjectListQuery {
                    limit: args.limit,
                    offset: args.offset,
                    order: args.order,
                },
            )
            .await
            .map_err(|err| format!("failed to list projects: {err}"))?;
            Ok(tool_success(serde_json::to_value(projects).unwrap()))
        }
        "get_project" => {
            let args = parse_tool_arguments::<ProjectByIdArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            let project = load_project_record(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to load project: {err}"))?;
            match project {
                Some(project) => Ok(tool_success(serde_json::to_value(project).unwrap())),
                None => Ok(tool_error(format!(
                    "project '{}' not found",
                    args.project_id
                ))),
            }
        }
        "list_project_pipelines" => {
            let args = parse_tool_arguments::<ProjectByIdArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            if !project_exists(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to load project: {err}"))?
            {
                return Ok(tool_error(format!(
                    "project '{}' not found",
                    args.project_id
                )));
            }
            let pipelines = load_pipelines_for_project(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to load project pipelines: {err}"))?;
            Ok(tool_success(serde_json::to_value(pipelines).unwrap()))
        }
        "list_project_specs" => {
            let args = parse_tool_arguments::<ProjectByIdArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            if !project_exists(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to load project: {err}"))?
            {
                return Ok(tool_error(format!(
                    "project '{}' not found",
                    args.project_id
                )));
            }
            let specs = list_project_spec_records(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to list project specs: {err}"))?;
            Ok(tool_success(serde_json::to_value(specs).unwrap()))
        }
        "validate_openapi" => {
            let args = parse_tool_arguments::<ValidateOpenApiToolArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            let payload = validate_openapi_source(&args.source);
            Ok(tool_success(serde_json::to_value(payload).unwrap()))
        }
        _ => Ok(tool_error(format!(
            "tool '{}' is not available",
            params.name
        ))),
    }
}

fn parse_params<T>(params: Option<Value>) -> Result<T, String>
where
    T: DeserializeOwned,
{
    match params {
        Some(value) => serde_json::from_value(value).map_err(|err| err.to_string()),
        None => Err("params are required".to_owned()),
    }
}

fn parse_optional_params<T>(params: Option<Value>) -> Result<T, String>
where
    T: DeserializeOwned + Default,
{
    match params {
        Some(value) => serde_json::from_value(value).map_err(|err| err.to_string()),
        None => Ok(T::default()),
    }
}

fn parse_tool_arguments<T>(arguments: Value) -> Result<T, String>
where
    T: DeserializeOwned,
{
    serde_json::from_value(arguments).map_err(|err| err.to_string())
}

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "health".to_owned(),
            title: Some("Health".to_owned()),
            description: "Returns a simple health payload for the orchestrator.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "get_info".to_owned(),
            title: Some("Runner Info".to_owned()),
            description: "Returns runner registration and health information.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "get_openapi_document".to_owned(),
            title: Some("OpenAPI Document".to_owned()),
            description: "Returns the orchestrator OpenAPI document.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "list_projects".to_owned(),
            title: Some("List Projects".to_owned()),
            description: "Lists projects stored in the orchestrator database.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 0 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "order": { "type": "string", "enum": ["asc", "desc"] }
                }
            }),
        },
        ToolDefinition {
            name: "get_project".to_owned(),
            title: Some("Get Project".to_owned()),
            description: "Returns a project by its id.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 }
                }
            }),
        },
        ToolDefinition {
            name: "list_project_pipelines".to_owned(),
            title: Some("List Pipelines".to_owned()),
            description: "Lists pipelines for a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 }
                }
            }),
        },
        ToolDefinition {
            name: "list_project_specs".to_owned(),
            title: Some("List Specs".to_owned()),
            description: "Lists OpenAPI specs associated with a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 }
                }
            }),
        },
        ToolDefinition {
            name: "validate_openapi".to_owned(),
            title: Some("Validate OpenAPI".to_owned()),
            description: "Validates an OpenAPI YAML or JSON document.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["source"],
                "properties": {
                    "source": { "type": "string", "minLength": 1 }
                }
            }),
        },
    ]
}

fn tool_success(value: Value) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolTextContent {
            kind: "text",
            text: serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        }],
        structured_content: Some(value),
        is_error: false,
    }
}

fn tool_error(message: String) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolTextContent {
            kind: "text",
            text: message,
        }],
        structured_content: None,
        is_error: true,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_tool_arguments, tool_definitions};
    use crate::server::mcp::models::ProjectByIdArgs;

    #[test]
    fn project_tools_require_project_id() {
        let tool = tool_definitions()
            .into_iter()
            .find(|tool| tool.name == "get_project")
            .expect("get_project tool definition");

        assert_eq!(tool.input_schema["required"], json!(["projectId"]));
    }

    #[test]
    fn parse_project_argument_payload() {
        let args = parse_tool_arguments::<ProjectByIdArgs>(json!({ "projectId": "abc" }))
            .expect("valid project args");

        assert_eq!(args.project_id, "abc");
    }

    #[test]
    fn parse_project_argument_payload_with_meta() {
        let args = parse_tool_arguments::<ProjectByIdArgs>(
            json!({ "projectId": "abc", "_meta": { "source": "client" } }),
        )
        .expect("valid project args with meta");

        assert_eq!(args.project_id, "abc");
        assert_eq!(args.meta, Some(json!({ "source": "client" })));
    }
}
