use previa_runner::Pipeline;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tracing::info;

use crate::server::db::{
    delete_pipeline_record, insert_project_pipeline, list_e2e_history_records,
    list_load_history_records, list_project_records, list_project_spec_records,
    load_e2e_history_record_by_id, load_load_history_record_by_id, load_pipelines_for_project,
    load_project_pipeline_record, load_project_record, project_exists, update_project_pipeline,
};
use crate::server::docs::build_openapi_document;
use crate::server::execution::collect_runner_statuses;
use crate::server::execution::resolve_runtime_specs_for_execution;
use crate::server::mcp::models::{
    CreateProjectPipelineArgs, InitializeParams, ListProjectsToolArgs, McpPeerInfo, McpRequest,
    McpResponse, McpSession, ProjectByIdArgs, ProjectHistoryToolArgs, ProjectPipelineByIdArgs,
    ProjectTestByIdArgs, PromptDefinition, PromptGetParams, PromptGetResult, PromptMessage,
    PromptTextContent, PromptsListParams, SUPPORTED_PROTOCOL_VERSIONS, ToolCallParams,
    ToolCallResult, ToolDefinition, ToolTextContent, ToolsListParams, UpdateProjectPipelineArgs,
    ValidateOpenApiToolArgs,
};
use crate::server::models::{HistoryQuery, OrchestratorInfoResponse, ProjectListQuery};
use crate::server::state::AppState;
use crate::server::utils::new_uuid_v7;
use crate::server::validation::openapi::validate_openapi_source;
use crate::server::validation::pipelines::{KNOWN_TEMPLATE_HELPERS, validate_pipeline_templates};

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
        "prompts/list" => {
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
            let params = match parse_optional_params::<PromptsListParams>(request.params) {
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
                response: McpResponse::success(
                    request_id,
                    json!({ "prompts": prompt_definitions() }),
                ),
                session_id: session_id.map(str::to_owned),
                protocol_version: Some(session.protocol_version),
            }
        }
        "prompts/get" => {
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
            let params = match parse_params::<PromptGetParams>(request.params) {
                Ok(params) => params,
                Err(message) => {
                    return McpHttpOutcome::Response {
                        response: McpResponse::error(Some(request_id), INVALID_PARAMS, message),
                        session_id: session_id.map(str::to_owned),
                        protocol_version: Some(session.protocol_version),
                    };
                }
            };
            let _ = &params.arguments;
            let _ = params.meta.as_ref();

            match prompt_result(&params.name) {
                Some(result) => McpHttpOutcome::Response {
                    response: McpResponse::success(
                        request_id,
                        serde_json::to_value(result).unwrap(),
                    ),
                    session_id: session_id.map(str::to_owned),
                    protocol_version: Some(session.protocol_version),
                },
                None => McpHttpOutcome::Response {
                    response: McpResponse::error(
                        Some(request_id),
                        INVALID_PARAMS,
                        format!("prompt '{}' is not available", params.name),
                    ),
                    session_id: session_id.map(str::to_owned),
                    protocol_version: Some(session.protocol_version),
                },
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
                    "prompts": {
                        "listChanged": false
                    },
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": McpPeerInfo {
                    name: env!("CARGO_PKG_NAME").to_owned(),
                    title: Some("Previa Main MCP".to_owned()),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                "instructions": "Use the available tools to inspect orchestrator health, projects, pipelines, OpenAPI specs, and to validate OpenAPI source content. Use the available prompts when you need operational guidance for creating pipelines, reviewing executed tests, and proposing step fixes."
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
                context: state.context_name.clone(),
                total_runners: runners.len(),
                active_runners: runners.iter().filter(|runner| runner.active).count(),
                runners,
            };
            Ok(tool_success(serde_json::to_value(payload).unwrap()))
        }
        "get_openapi_document" => Ok(tool_success(
            serde_json::to_value(build_openapi_document()).unwrap(),
        )),
        "get_pipeline_creation_guide" => Ok(tool_success(pipeline_creation_guide())),
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
        "list_e2e_history" => {
            let args = parse_tool_arguments::<ProjectHistoryToolArgs>(params.arguments)?;
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
            let records = list_e2e_history_records(
                &state.db,
                &args.project_id,
                HistoryQuery {
                    pipeline_index: args.pipeline_index,
                    limit: args.limit,
                    offset: args.offset,
                    order: args.order,
                },
            )
            .await
            .map_err(|err| format!("failed to list e2e history: {err}"))?;
            Ok(tool_success(serde_json::to_value(records).unwrap()))
        }
        "get_e2e_test" => {
            let args = parse_tool_arguments::<ProjectTestByIdArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            let record = load_e2e_history_record_by_id(&state.db, &args.project_id, &args.test_id)
                .await
                .map_err(|err| format!("failed to load e2e test: {err}"))?;
            match record {
                Some(record) => Ok(tool_success(serde_json::to_value(record).unwrap())),
                None => Ok(tool_error(format!(
                    "e2e test '{}' not found in project '{}'",
                    args.test_id, args.project_id
                ))),
            }
        }
        "list_load_history" => {
            let args = parse_tool_arguments::<ProjectHistoryToolArgs>(params.arguments)?;
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
            let records = list_load_history_records(
                &state.db,
                &args.project_id,
                HistoryQuery {
                    pipeline_index: args.pipeline_index,
                    limit: args.limit,
                    offset: args.offset,
                    order: args.order,
                },
            )
            .await
            .map_err(|err| format!("failed to list load history: {err}"))?;
            Ok(tool_success(serde_json::to_value(records).unwrap()))
        }
        "get_load_test" => {
            let args = parse_tool_arguments::<ProjectTestByIdArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            let record = load_load_history_record_by_id(&state.db, &args.project_id, &args.test_id)
                .await
                .map_err(|err| format!("failed to load load test: {err}"))?;
            match record {
                Some(record) => Ok(tool_success(serde_json::to_value(record).unwrap())),
                None => Ok(tool_error(format!(
                    "load test '{}' not found in project '{}'",
                    args.test_id, args.project_id
                ))),
            }
        }
        "get_project_pipeline" => {
            let args = parse_tool_arguments::<ProjectPipelineByIdArgs>(params.arguments)?;
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
            let pipeline =
                load_project_pipeline_record(&state.db, &args.project_id, &args.pipeline_id)
                    .await
                    .map_err(|err| format!("failed to load project pipeline: {err}"))?;
            match pipeline {
                Some(pipeline) => Ok(tool_success(serde_json::to_value(pipeline).unwrap())),
                None => Ok(tool_error(format!(
                    "pipeline '{}' not found in project '{}'",
                    args.pipeline_id, args.project_id
                ))),
            }
        }
        "create_project_pipeline" => {
            let args = parse_tool_arguments::<CreateProjectPipelineArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            validate_pipeline_input(&args.pipeline)?;
            let runtime_specs =
                resolve_runtime_specs_for_execution(&state.db, Some(&args.project_id), &[])
                    .await
                    .map_err(|err| format!("failed to load project specs for validation: {err}"))?;
            let template_errors =
                validate_pipeline_templates(&args.pipeline, runtime_specs.as_deref());
            if !template_errors.is_empty() {
                return Ok(tool_error(template_errors.join("; ")));
            }
            if !project_exists(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to load project: {err}"))?
            {
                return Ok(tool_error(format!(
                    "project '{}' not found",
                    args.project_id
                )));
            }
            let pipeline = insert_project_pipeline(&state.db, &args.project_id, args.pipeline)
                .await
                .map_err(|err| format!("failed to create project pipeline: {err}"))?;
            Ok(tool_success(serde_json::to_value(pipeline).unwrap()))
        }
        "update_project_pipeline" => {
            let args = parse_tool_arguments::<UpdateProjectPipelineArgs>(params.arguments)?;
            let _ = args.meta.as_ref();
            validate_pipeline_input(&args.pipeline)?;
            let runtime_specs =
                resolve_runtime_specs_for_execution(&state.db, Some(&args.project_id), &[])
                    .await
                    .map_err(|err| format!("failed to load project specs for validation: {err}"))?;
            let template_errors =
                validate_pipeline_templates(&args.pipeline, runtime_specs.as_deref());
            if !template_errors.is_empty() {
                return Ok(tool_error(template_errors.join("; ")));
            }
            if !project_exists(&state.db, &args.project_id)
                .await
                .map_err(|err| format!("failed to load project: {err}"))?
            {
                return Ok(tool_error(format!(
                    "project '{}' not found",
                    args.project_id
                )));
            }
            let pipeline = update_project_pipeline(
                &state.db,
                &args.project_id,
                &args.pipeline_id,
                args.pipeline,
            )
            .await
            .map_err(|err| format!("failed to update project pipeline: {err}"))?;
            match pipeline {
                Some(pipeline) => Ok(tool_success(serde_json::to_value(pipeline).unwrap())),
                None => Ok(tool_error(format!(
                    "pipeline '{}' not found in project '{}'",
                    args.pipeline_id, args.project_id
                ))),
            }
        }
        "delete_project_pipeline" => {
            let args = parse_tool_arguments::<ProjectPipelineByIdArgs>(params.arguments)?;
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
            let deleted = delete_pipeline_record(&state.db, &args.project_id, &args.pipeline_id)
                .await
                .map_err(|err| format!("failed to delete project pipeline: {err}"))?;
            if deleted {
                Ok(tool_success(json!({
                    "projectId": args.project_id,
                    "pipelineId": args.pipeline_id,
                    "deleted": true
                })))
            } else {
                Ok(tool_error(format!(
                    "pipeline '{}' not found in project '{}'",
                    args.pipeline_id, args.project_id
                )))
            }
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
            name: "get_pipeline_creation_guide".to_owned(),
            title: Some("Pipeline Guide".to_owned()),
            description:
                "Explains how to create a pipeline, with examples and supported template variables."
                    .to_owned(),
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
            name: "list_e2e_history".to_owned(),
            title: Some("List E2E History".to_owned()),
            description: "Lists executed E2E tests for a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "pipelineIndex": { "type": "integer" },
                    "limit": { "type": "integer", "minimum": 1 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "order": { "type": "string", "enum": ["asc", "desc"] }
                }
            }),
        },
        ToolDefinition {
            name: "get_e2e_test".to_owned(),
            title: Some("Get E2E Test".to_owned()),
            description: "Returns a single executed E2E test by history id or execution id."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId", "testId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "testId": { "type": "string", "minLength": 1 }
                }
            }),
        },
        ToolDefinition {
            name: "list_load_history".to_owned(),
            title: Some("List Load History".to_owned()),
            description: "Lists executed load tests for a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "pipelineIndex": { "type": "integer" },
                    "limit": { "type": "integer", "minimum": 1 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "order": { "type": "string", "enum": ["asc", "desc"] }
                }
            }),
        },
        ToolDefinition {
            name: "get_load_test".to_owned(),
            title: Some("Get Load Test".to_owned()),
            description: "Returns a single executed load test by history id or execution id."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId", "testId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "testId": { "type": "string", "minLength": 1 }
                }
            }),
        },
        ToolDefinition {
            name: "get_project_pipeline".to_owned(),
            title: Some("Get Pipeline".to_owned()),
            description: "Returns a single pipeline from a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId", "pipelineId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "pipelineId": { "type": "string", "minLength": 1 }
                }
            }),
        },
        ToolDefinition {
            name: "create_project_pipeline".to_owned(),
            title: Some("Create Pipeline".to_owned()),
            description: "Creates a pipeline inside a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId", "pipeline"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "pipeline": pipeline_schema()
                }
            }),
        },
        ToolDefinition {
            name: "update_project_pipeline".to_owned(),
            title: Some("Update Pipeline".to_owned()),
            description: "Updates an existing pipeline in a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId", "pipelineId", "pipeline"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "pipelineId": { "type": "string", "minLength": 1 },
                    "pipeline": pipeline_schema()
                }
            }),
        },
        ToolDefinition {
            name: "delete_project_pipeline".to_owned(),
            title: Some("Delete Pipeline".to_owned()),
            description: "Deletes a pipeline from a project.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["projectId", "pipelineId"],
                "properties": {
                    "projectId": { "type": "string", "minLength": 1 },
                    "pipelineId": { "type": "string", "minLength": 1 }
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

fn prompt_definitions() -> Vec<PromptDefinition> {
    vec![
        PromptDefinition {
            name: "default".to_owned(),
            title: Some("Default".to_owned()),
            description: Some(
                "Guides the LLM to create pipelines, evaluate executed tests and steps, and propose safe fixes before applying any change."
                    .to_owned(),
            ),
            arguments: Vec::new(),
        },
        PromptDefinition {
            name: "pipeline_creation_specialist".to_owned(),
            title: Some("Pipeline Creation Specialist".to_owned()),
            description: Some(
                "Detailed prompt for creating valid Previa pipelines with schemas, template variables, rules, and examples."
                    .to_owned(),
            ),
            arguments: Vec::new(),
        },
    ]
}

fn prompt_result(name: &str) -> Option<PromptGetResult> {
    match name {
        "default" | "pipeline_test_assistant" => Some(PromptGetResult {
            description: Some(
                "Operational prompt for pipeline authoring, test analysis, and step repair."
                    .to_owned(),
            ),
            messages: vec![PromptMessage {
                role: "user".to_owned(),
                content: PromptTextContent {
                    kind: "text",
                    text: pipeline_test_assistant_prompt(),
                },
            }],
        }),
        "pipeline_creation_specialist" => Some(PromptGetResult {
            description: Some(
                "Detailed prompt for authoring Previa pipelines through MCP.".to_owned(),
            ),
            messages: vec![PromptMessage {
                role: "user".to_owned(),
                content: PromptTextContent {
                    kind: "text",
                    text: pipeline_creation_specialist_prompt(),
                },
            }],
        }),
        _ => None,
    }
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

fn validate_pipeline_input(pipeline: &Pipeline) -> Result<(), String> {
    if pipeline.name.trim().is_empty() {
        return Err("pipeline name is required".to_owned());
    }
    if pipeline.steps.is_empty() {
        return Err("pipeline must contain at least one step".to_owned());
    }
    Ok(())
}

fn pipeline_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "steps"],
        "properties": {
            "id": { "type": "string" },
            "name": { "type": "string", "minLength": 1 },
            "description": { "type": ["string", "null"] },
            "steps": {
                "type": "array",
                "minItems": 1,
                "items": pipeline_step_schema()
            }
        }
    })
}

fn pipeline_step_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "name", "method", "url"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 },
            "description": { "type": ["string", "null"] },
            "method": { "type": "string", "minLength": 1 },
            "url": { "type": "string", "minLength": 1 },
            "headers": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            },
            "body": {},
            "operationId": { "type": ["string", "null"] },
            "delay": { "type": ["integer", "null"], "minimum": 0 },
            "retry": { "type": ["integer", "null"], "minimum": 0 },
            "asserts": {
                "type": "array",
                "items": assertion_schema()
            }
        }
    })
}

fn assertion_schema() -> Value {
    json!({
        "type": "object",
        "required": ["field", "operator"],
        "properties": {
            "field": { "type": "string", "minLength": 1 },
            "operator": { "type": "string", "minLength": 1 },
            "expected": { "type": ["string", "null"] }
        }
    })
}

fn pipeline_creation_guide() -> Value {
    json!({
        "workflow": [
            "1. Call list_projects or get_project to choose the target project.",
            "2. Optionally call list_project_specs to inspect available spec slugs and base URL names for template usage.",
            "3. Build a pipeline object with name, optional description, and at least one step.",
            "4. Use create_project_pipeline with projectId + pipeline.",
            "5. Before execution, templates are validated. Unknown variables like {{run.id}} are rejected."
        ],
        "createTool": "create_project_pipeline",
        "updateTool": "update_project_pipeline",
        "pipelineRules": [
            "pipeline.name is required",
            "pipeline.steps must contain at least one step",
            "each step requires id, name, method, and url",
            "steps.<stepId> references can only target steps that already ran earlier in the same pipeline",
            "specs.<slug>.url.<name> references only work when the project has matching runtime specs configured",
            "supported template locations include step url, headers, body, and assertion expected values"
        ],
        "supportedTemplateVariables": {
            "steps": {
                "pattern": "{{steps.<stepId>.<fieldPath>}}",
                "description": "Reads values from the response body of a previous step.",
                "example": "{{steps.login.token}}"
            },
            "specs": {
                "pattern": "{{specs.<slug>.url.<name>}}",
                "description": "Reads base URLs from runtime specs attached to the project or provided for execution.",
                "example": "{{specs.payments.url.hml}}"
            },
            "helpers": KNOWN_TEMPLATE_HELPERS,
            "helperExamples": [
                "{{helpers.uuid}}",
                "{{helpers.email}}",
                "{{helpers.name}}",
                "{{helpers.username}}",
                "{{helpers.number 1 100}}",
                "{{helpers.date}}",
                "{{helpers.boolean}}",
                "{{helpers.cpf}}"
            ],
            "unsupportedExamples": [
                "{{run.id}}",
                "{{project.id}}",
                "{{pipeline.id}}",
                "{{env.API_URL}}"
            ]
        },
        "exampleCreateProjectPipelineArguments": {
            "projectId": "project_123",
            "pipeline": {
                "name": "Create And Fetch User",
                "description": "Creates a user and then fetches it using the id returned by the first step.",
                "steps": [
                    {
                        "id": "create_user",
                        "name": "Create user",
                        "method": "POST",
                        "url": "{{specs.users.url.hml}}/users",
                        "headers": {
                            "content-type": "application/json",
                            "x-request-id": "{{helpers.uuid}}"
                        },
                        "body": {
                            "name": "{{helpers.name}}",
                            "email": "{{helpers.email}}"
                        },
                        "asserts": [
                            {
                                "field": "status",
                                "operator": "equals",
                                "expected": "201"
                            }
                        ]
                    },
                    {
                        "id": "get_user",
                        "name": "Get user",
                        "method": "GET",
                        "url": "{{specs.users.url.hml}}/users/{{steps.create_user.id}}",
                        "headers": {},
                        "asserts": [
                            {
                                "field": "status",
                                "operator": "equals",
                                "expected": "200"
                            },
                            {
                                "field": "body.email",
                                "operator": "equals",
                                "expected": "{{steps.create_user.email}}"
                            }
                        ]
                    }
                ]
            }
        }
    })
}

fn pipeline_test_assistant_prompt() -> String {
    [
        "You are responsible for operating Previa pipelines through the MCP server.",
        "Your job has three parts: create pipelines, evaluate executed tests and step results, and fix broken steps when needed.",
        "When creating pipelines, prefer this workflow: inspect the project, inspect project specs, call get_pipeline_creation_guide, build the pipeline, then call create_project_pipeline or update_project_pipeline.",
        "Use only supported template variables. Valid roots are steps.<stepId>.*, specs.<slug>.url.<name>, and helpers.*. Do not invent variables such as run.id, project.id, pipeline.id, or env.*.",
        "When evaluating tests, inspect list_e2e_history or list_load_history first, then use get_e2e_test or get_load_test to analyze the exact execution details, including request, response, body, asserts, and step-level failures when available.",
        "When a step fails, identify the most likely root cause from the execution data before suggesting any change. Consider status assertions, request body mistakes, wrong URLs, missing headers, invalid template references, and downstream dependency errors.",
        "When you find a problem, always propose a concrete solution first and then ask the user if they want you to apply it. Do not silently modify a pipeline without explicit user confirmation.",
        "Your proposed solution should be specific. Name the failing step, explain the issue, show the exact change you want to make, and mention which MCP tool you will use to apply it.",
        "If the current data is insufficient to justify a fix, say what is missing and which MCP tool should be called next.",
        "When the user approves a change, update the saved pipeline with update_project_pipeline instead of inventing a non-existent tool name.",
        "When discussing a new pipeline, provide a valid example that matches the input schema accepted by create_project_pipeline.",
    ]
    .join("\n")
}

fn pipeline_creation_specialist_prompt() -> String {
    let pipeline_schema = serde_json::to_string_pretty(&pipeline_schema()).unwrap();
    let step_schema = serde_json::to_string_pretty(&pipeline_step_schema()).unwrap();
    let assertion_schema = serde_json::to_string_pretty(&assertion_schema()).unwrap();
    let example_payload = serde_json::to_string_pretty(
        &pipeline_creation_guide()["exampleCreateProjectPipelineArguments"],
    )
    .unwrap();

    format!(
        "You are responsible for creating valid Previa pipelines through MCP.\n\
Your goal is to produce payloads that can be sent directly to create_project_pipeline or update_project_pipeline.\n\
\n\
Required workflow:\n\
1. Identify the target project with list_projects or get_project.\n\
2. Inspect the project's runtime specs with list_project_specs before using specs.<slug>.url.<name> variables.\n\
3. Build a pipeline object that matches the schemas below.\n\
4. Prefer explicit status asserts on every HTTP step.\n\
5. Return a final payload compatible with create_project_pipeline.\n\
\n\
Creation rules:\n\
- Always use create_project_pipeline to create a new saved pipeline.\n\
- Always use update_project_pipeline to modify an existing saved pipeline.\n\
- Do not invent non-existent tools such as save_pipeline.\n\
- pipeline.name is required.\n\
- pipeline.steps must contain at least one step.\n\
- Each step requires id, name, method, and url.\n\
- steps.<stepId> references can only point to steps that ran earlier in the same pipeline.\n\
- Supported template locations include url, headers, body, and assertion expected values.\n\
- Unknown variables like {{{{run.id}}}}, {{{{project.id}}}}, {{{{pipeline.id}}}}, and {{{{env.API_URL}}}} are invalid.\n\
\n\
Supported template variables:\n\
- Previous step response body: {{{{steps.<stepId>.<fieldPath>}}}}\n\
  Example: {{{{steps.login.token}}}}\n\
- Runtime spec base URLs: {{{{specs.<slug>.url.<name>}}}}\n\
  Example: {{{{specs.payments.url.hml}}}}\n\
- Helpers:\n\
  - {{{{helpers.uuid}}}}\n\
  - {{{{helpers.email}}}}\n\
  - {{{{helpers.name}}}}\n\
  - {{{{helpers.username}}}}\n\
  - {{{{helpers.number 1 100}}}}\n\
  - {{{{helpers.date}}}}\n\
  - {{{{helpers.boolean}}}}\n\
  - {{{{helpers.cpf}}}}\n\
\n\
Schema for pipeline:\n\
```json\n\
{pipeline_schema}\n\
```\n\
\n\
Schema for step:\n\
```json\n\
{step_schema}\n\
```\n\
\n\
Schema for assertion:\n\
```json\n\
{assertion_schema}\n\
```\n\
\n\
Recommended authoring guidance:\n\
- Use stable, descriptive step ids because later steps depend on them.\n\
- Keep request headers explicit, especially content-type when sending JSON.\n\
- Add status assertions to every step.\n\
- When validating response bodies, use body.<field> assertions.\n\
- When chaining steps, reference values from previous response bodies with steps.<stepId>.<fieldPath>.\n\
- If a pipeline depends on project specs, verify the slug and URL name from list_project_specs before generating the payload.\n\
\n\
Example payload for create_project_pipeline:\n\
```json\n\
{example_payload}\n\
```\n\
\n\
Output requirements:\n\
- Return a valid JSON object for create_project_pipeline arguments.\n\
- Keep the payload directly executable by MCP.\n\
- If required project or spec information is missing, say exactly which MCP tool should be called next instead of guessing.\n"
    )
}

#[cfg(test)]
mod tests {
    use previa_runner::{Pipeline, PipelineStep};
    use serde_json::json;

    use super::{
        parse_tool_arguments, pipeline_creation_guide, pipeline_creation_specialist_prompt,
        pipeline_test_assistant_prompt, prompt_definitions, prompt_result, tool_definitions,
        validate_pipeline_input,
    };
    use crate::server::mcp::models::{
        CreateProjectPipelineArgs, ProjectByIdArgs, ProjectHistoryToolArgs,
    };

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

    #[test]
    fn parse_create_pipeline_arguments() {
        let args = parse_tool_arguments::<CreateProjectPipelineArgs>(json!({
            "projectId": "project-1",
            "pipeline": {
                "name": "Pipeline A",
                "description": null,
                "steps": [
                    {
                        "id": "step-1",
                        "name": "Step 1",
                        "method": "GET",
                        "url": "https://example.com",
                        "headers": {},
                        "asserts": []
                    }
                ]
            }
        }))
        .expect("valid create pipeline args");

        assert_eq!(args.project_id, "project-1");
        assert_eq!(args.pipeline.name, "Pipeline A");
    }

    #[test]
    fn validate_pipeline_requires_name() {
        let pipeline = Pipeline {
            id: None,
            name: "   ".to_owned(),
            description: None,
            steps: vec![PipelineStep {
                id: "step-1".to_owned(),
                name: "Step 1".to_owned(),
                description: None,
                method: "GET".to_owned(),
                url: "https://example.com".to_owned(),
                headers: Default::default(),
                body: None,
                operation_id: None,
                delay: None,
                retry: None,
                asserts: Vec::new(),
            }],
        };

        assert_eq!(
            validate_pipeline_input(&pipeline).expect_err("pipeline name should be validated"),
            "pipeline name is required"
        );
    }

    #[test]
    fn pipeline_guide_tool_is_available() {
        let tool = tool_definitions()
            .into_iter()
            .find(|tool| tool.name == "get_pipeline_creation_guide")
            .expect("pipeline guide tool definition");

        assert_eq!(tool.input_schema["type"], json!("object"));
    }

    #[test]
    fn pipeline_guide_mentions_unsupported_run_id() {
        let guide = pipeline_creation_guide();

        assert!(
            guide["supportedTemplateVariables"]["unsupportedExamples"]
                .as_array()
                .expect("unsupported examples array")
                .iter()
                .any(|value| value == "{{run.id}}")
        );
    }

    #[test]
    fn pipeline_prompt_is_available() {
        let prompt = prompt_definitions()
            .into_iter()
            .find(|prompt| prompt.name == "default")
            .expect("pipeline prompt definition");

        assert_eq!(prompt.arguments.len(), 0);
    }

    #[test]
    fn pipeline_prompt_mentions_pipeline_creation_tool_and_confirmation() {
        let prompt = prompt_result("default").expect("pipeline prompt");
        let text = &prompt.messages[0].content.text;

        assert!(text.contains("create_project_pipeline"));
        assert!(text.contains("ask the user if they want you to apply it"));
    }

    #[test]
    fn legacy_pipeline_prompt_alias_is_still_available() {
        let prompt = prompt_result("pipeline_test_assistant").expect("legacy pipeline prompt");

        assert!(
            prompt.messages[0]
                .content
                .text
                .contains("create_project_pipeline")
        );
    }

    #[test]
    fn pipeline_prompt_mentions_execution_analysis_tools() {
        let text = pipeline_test_assistant_prompt();

        assert!(text.contains("get_e2e_test"));
        assert!(text.contains("get_load_test"));
        assert!(text.contains("update_project_pipeline"));
    }

    #[test]
    fn pipeline_creation_prompt_is_available() {
        let prompt = prompt_definitions()
            .into_iter()
            .find(|prompt| prompt.name == "pipeline_creation_specialist")
            .expect("pipeline creation prompt definition");

        assert_eq!(prompt.arguments.len(), 0);
    }

    #[test]
    fn pipeline_creation_prompt_mentions_schema_variables_and_examples() {
        let text = pipeline_creation_specialist_prompt();

        assert!(text.contains("Schema for pipeline"));
        assert!(text.contains("{{steps.<stepId>.<fieldPath>}}"));
        assert!(text.contains("{{specs.<slug>.url.<name>}}"));
        assert!(text.contains("Example payload for create_project_pipeline"));
        assert!(text.contains("save_pipeline"));
    }

    #[test]
    fn pipeline_creation_prompt_result_is_available() {
        let prompt =
            prompt_result("pipeline_creation_specialist").expect("pipeline creation prompt result");

        assert!(
            prompt.messages[0]
                .content
                .text
                .contains("create_project_pipeline")
        );
    }

    #[test]
    fn parse_history_arguments() {
        let args = parse_tool_arguments::<ProjectHistoryToolArgs>(json!({
            "projectId": "project-1",
            "pipelineIndex": 2,
            "limit": 50,
            "offset": 0,
            "order": "desc"
        }))
        .expect("valid history args");

        assert_eq!(args.project_id, "project-1");
        assert_eq!(args.pipeline_index, Some(2));
        assert_eq!(args.limit, Some(50));
    }
}
