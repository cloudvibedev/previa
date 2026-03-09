use serde_json::Value;
use std::collections::HashMap;

use crate::core::types::{AssertionResult, PipelineStep, RuntimeSpec, StepExecutionResult};
use crate::template::resolve::{resolve_template_variables, value_to_string};

pub(crate) fn has_status_assertion(step: &PipelineStep) -> bool {
    step.asserts.iter().any(|assertion| assertion.field == "status")
}

pub(crate) fn resolve_assert_field(field: &str, result: &StepExecutionResult) -> Option<String> {
    let response = result.response.as_ref()?;

    if field == "status" {
        return Some(response.status.to_string());
    }

    if let Some(path) = field.strip_prefix("body.") {
        let mut current = &response.body;
        for key in path.split('.') {
            current = match current {
                Value::Object(map) => map.get(key)?,
                _ => return None,
            };
        }
        return value_to_string(current);
    }

    if let Some(header_name) = field.strip_prefix("header.") {
        for (k, v) in &response.headers {
            if k.eq_ignore_ascii_case(header_name) {
                return Some(v.clone());
            }
        }
    }

    None
}

pub(crate) fn evaluate_assertions(
    step: &PipelineStep,
    result: &StepExecutionResult,
    context: &HashMap<String, StepExecutionResult>,
    specs: Option<&[RuntimeSpec]>,
) -> Vec<AssertionResult> {
    step.asserts
        .iter()
        .map(|assertion| {
            let actual = resolve_assert_field(&assertion.field, result);
            let expected = assertion.expected.as_ref().map(|exp| {
                resolve_template_variables(&Value::String(exp.clone()), context, specs)
                    .as_str()
                    .unwrap_or(exp)
                    .to_owned()
            });

            let passed = match assertion.operator.as_str() {
                "equals" => actual == expected,
                "not_equals" => actual != expected,
                "contains" => match (actual.as_ref(), expected.as_ref()) {
                    (Some(a), Some(e)) => a.contains(e),
                    _ => false,
                },
                "exists" => actual.is_some(),
                "not_exists" => actual.is_none(),
                "gt" => match (actual.as_ref(), expected.as_ref()) {
                    (Some(a), Some(e)) => {
                        let left = a.parse::<f64>().ok();
                        let right = e.parse::<f64>().ok();
                        matches!((left, right), (Some(l), Some(r)) if l > r)
                    }
                    _ => false,
                },
                "lt" => match (actual.as_ref(), expected.as_ref()) {
                    (Some(a), Some(e)) => {
                        let left = a.parse::<f64>().ok();
                        let right = e.parse::<f64>().ok();
                        matches!((left, right), (Some(l), Some(r)) if l < r)
                    }
                    _ => false,
                },
                _ => false,
            };

            AssertionResult {
                assertion: assertion.clone(),
                passed,
                actual,
            }
        })
        .collect()
}
