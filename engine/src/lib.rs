mod assertions;
mod core;
mod execution;
mod template;

use std::collections::HashMap;

use serde_json::Value;

pub use core::types::{
    AssertionResult, Pipeline, PipelineStep, RuntimeSpec, StepAssertion, StepExecutionResult,
    StepRequest, StepResponse,
};
pub use execution::{
    execute_pipeline, execute_pipeline_with_client, execute_pipeline_with_client_hooks,
    execute_pipeline_with_hooks, execute_pipeline_with_specs_hooks,
};

pub fn render_template_value(
    value: &Value,
    context: &HashMap<String, StepExecutionResult>,
    specs: Option<&[RuntimeSpec]>,
) -> Value {
    template::resolve::resolve_template_variables(value, context, specs)
}

pub fn render_template_value_simple(value: &Value) -> Value {
    let context = HashMap::<String, StepExecutionResult>::new();
    render_template_value(value, &context, None)
}
