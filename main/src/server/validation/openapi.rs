use std::collections::{HashMap, HashSet};

use oas3::OpenApiV3Spec;
use serde_json::Value;

use crate::server::models::{
    OpenApiValidationPoint, OpenApiValidationResponse, OpenApiValidationSeverity,
    OpenApiValidationStatus,
};

pub fn validate_openapi_source(source: &str) -> OpenApiValidationResponse {
    let source_md5 = format!("{:x}", md5::compute(source.as_bytes()));

    if source.trim().is_empty() {
        return OpenApiValidationResponse {
            spec: None,
            source_md5,
            status: OpenApiValidationStatus::Invalid,
            points: vec![OpenApiValidationPoint {
                severity: OpenApiValidationSeverity::Error,
                line: Some(1),
                pointer: None,
                comment: "O conteúdo do spec está vazio.".to_owned(),
            }],
        };
    }

    let parsed_value = parse_source_value(source);
    let parsed_spec = parse_openapi_spec(source);

    let (spec, mut points) = match parsed_spec {
        Ok(spec) => {
            let spec_value = serde_json::to_value(&spec).ok();
            let mut points = validate_semantics(&spec, source, spec_value.as_ref());
            if let Some(spec_value) = spec_value.as_ref() {
                validate_local_references(spec_value, source, &mut points);
            }
            (spec_value.or(parsed_value), points)
        }
        Err(parse_point) => (parsed_value, vec![parse_point]),
    };

    sort_and_dedupe_points(&mut points);
    let status = if points
        .iter()
        .any(|point| matches!(point.severity, OpenApiValidationSeverity::Error))
    {
        OpenApiValidationStatus::Invalid
    } else {
        OpenApiValidationStatus::Valid
    };

    OpenApiValidationResponse {
        spec,
        source_md5,
        status,
        points,
    }
}

fn parse_source_value(source: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(source) {
        return Some(value);
    }

    let yaml_value = serde_yaml::from_str::<serde_yaml::Value>(source).ok()?;
    serde_json::to_value(yaml_value).ok()
}

fn parse_openapi_spec(source: &str) -> Result<OpenApiV3Spec, OpenApiValidationPoint> {
    match oas3::from_json(source) {
        Ok(spec) => Ok(spec),
        Err(json_err) => match oas3::from_yaml(source) {
            Ok(spec) => Ok(spec),
            Err(yaml_err) => {
                let yaml_line = yaml_err.location().map(|loc| loc.line() as u32);
                let json_line = Some(json_err.line() as u32).filter(|line| *line > 0);
                let line = json_line.or(yaml_line);
                let comment = format!(
                    "Não foi possível interpretar o conteúdo como OpenAPI 3.1 (JSON/YAML). JSON: {}. YAML: {}.",
                    json_err, yaml_err
                );
                Err(error_point(line, None, comment))
            }
        },
    }
}

fn validate_semantics(
    spec: &OpenApiV3Spec,
    source: &str,
    spec_value: Option<&Value>,
) -> Vec<OpenApiValidationPoint> {
    let mut points = Vec::new();

    if let Err(err) = spec.validate_version() {
        points.push(error_point(
            find_line_for_key(source, "openapi"),
            Some("/openapi".to_owned()),
            format!(
                "Campo `openapi` inválido para o validador atual: {}. Use uma versão 3.1.x.",
                err
            ),
        ));
    }

    if spec.info.title.trim().is_empty() {
        points.push(error_point(
            find_line_for_key(source, "title"),
            Some("/info/title".to_owned()),
            "Campo `info.title` está vazio. Defina um título para a API.".to_owned(),
        ));
    }

    if spec.info.version.trim().is_empty() {
        points.push(error_point(
            find_line_for_key(source, "version"),
            Some("/info/version".to_owned()),
            "Campo `info.version` está vazio. Defina a versão da API.".to_owned(),
        ));
    }

    if spec.paths.as_ref().is_none_or(|paths| paths.is_empty()) {
        points.push(warning_point(
            find_line_for_key(source, "paths"),
            Some("/paths".to_owned()),
            "O spec não possui `paths`. Confirme se isso é intencional.".to_owned(),
        ));
    }

    let mut operation_ids: HashMap<String, Vec<String>> = HashMap::new();
    for (path, method, operation) in spec.operations() {
        let method_name = method.as_str().to_ascii_lowercase();
        let op_pointer = format!("/paths/{}/{}", escape_pointer_token(&path), method_name);

        if operation
            .responses
            .as_ref()
            .is_none_or(|responses| responses.is_empty())
        {
            points.push(error_point(
                find_line_for_pointer(source, &format!("{op_pointer}/responses")),
                Some(format!("{op_pointer}/responses")),
                format!(
                    "A operação {} {} não possui responses definidos.",
                    method, path
                ),
            ));
        }

        if let Some(operation_id) = operation.operation_id.as_ref() {
            let normalized = operation_id.trim().to_owned();
            if normalized.is_empty() {
                points.push(error_point(
                    find_line_for_pointer(source, &format!("{op_pointer}/operationId")),
                    Some(format!("{op_pointer}/operationId")),
                    format!("A operação {} {} tem `operationId` vazio.", method, path),
                ));
            } else {
                operation_ids
                    .entry(normalized)
                    .or_default()
                    .push(format!("{} {}", method, path));
            }
        }
    }

    for (operation_id, uses) in operation_ids {
        if uses.len() <= 1 {
            continue;
        }

        points.push(error_point(
            find_line_for_field_value(source, "operationId", &operation_id),
            None,
            format!(
                "operationId duplicado (`{}`). Aparece em: {}.",
                operation_id,
                uses.join(", ")
            ),
        ));
    }

    if spec_value.is_none() {
        points.push(warning_point(
            None,
            None,
            "Não foi possível serializar o spec parseado para validar referências locais."
                .to_owned(),
        ));
    }

    points
}

fn validate_local_references(
    spec_value: &Value,
    source: &str,
    points: &mut Vec<OpenApiValidationPoint>,
) {
    let mut refs = Vec::new();
    collect_refs(spec_value, "", &mut refs);

    let mut seen = HashSet::new();
    for (pointer, target) in refs {
        let dedupe_key = format!("{pointer}|{target}");
        if !seen.insert(dedupe_key) {
            continue;
        }

        if target.starts_with("#/") {
            let target_pointer = &target[1..];
            if spec_value.pointer(target_pointer).is_none() {
                points.push(error_point(
                    find_line_for_ref(source, &target)
                        .or_else(|| find_line_for_pointer(source, &pointer)),
                    Some(pointer),
                    format!(
                        "Referência local inválida: `{}` não existe no documento.",
                        target
                    ),
                ));
            }
            continue;
        }

        if target.starts_with('#') {
            points.push(error_point(
                find_line_for_ref(source, &target)
                    .or_else(|| find_line_for_pointer(source, &pointer)),
                Some(pointer),
                format!(
                    "Formato de referência local inválido (`{}`). Use `#/...`.",
                    target
                ),
            ));
            continue;
        }

        points.push(warning_point(
            find_line_for_ref(source, &target).or_else(|| find_line_for_pointer(source, &pointer)),
            Some(pointer),
            format!(
                "Referência externa (`{}`) não foi resolvida nesta validação local.",
                target
            ),
        ));
    }
}

fn collect_refs(value: &Value, pointer: &str, refs: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_pointer = format!("{}/{}", pointer, escape_pointer_token(key));
                if key == "$ref"
                    && let Some(target) = child.as_str()
                {
                    refs.push((child_pointer.clone(), target.to_owned()));
                }
                collect_refs(child, &child_pointer, refs);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let child_pointer = format!("{}/{}", pointer, index);
                collect_refs(child, &child_pointer, refs);
            }
        }
        _ => {}
    }
}

fn find_line_for_pointer(source: &str, pointer: &str) -> Option<u32> {
    let token = pointer
        .split('/')
        .filter(|segment| !segment.is_empty())
        .next_back()
        .map(unescape_pointer_token)?;
    find_line_for_key(source, &token)
}

fn find_line_for_key(source: &str, key: &str) -> Option<u32> {
    let json_pattern = format!("\"{key}\"");
    let yaml_pattern = format!("{key}:");

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if line.contains(&json_pattern) || trimmed.starts_with(&yaml_pattern) {
            return Some((index + 1) as u32);
        }
    }

    None
}

fn find_line_for_ref(source: &str, reference: &str) -> Option<u32> {
    for (index, line) in source.lines().enumerate() {
        if line.contains("$ref") && line.contains(reference) {
            return Some((index + 1) as u32);
        }
    }
    None
}

fn find_line_for_field_value(source: &str, field: &str, value: &str) -> Option<u32> {
    let field_json = format!("\"{field}\"");
    let value_json = format!("\"{value}\"");

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if line.contains(&field_json) && line.contains(&value_json) {
            return Some((index + 1) as u32);
        }
        if trimmed.starts_with(&format!("{field}:")) && line.contains(value) {
            return Some((index + 1) as u32);
        }
    }

    None
}

fn sort_and_dedupe_points(points: &mut Vec<OpenApiValidationPoint>) {
    points.sort_by_key(|point| {
        (
            point.line.unwrap_or(u32::MAX),
            severity_rank(point.severity),
        )
    });
    let mut seen = HashSet::new();
    points.retain(|point| {
        let key = format!(
            "{:?}|{:?}|{:?}|{}",
            point.severity, point.line, point.pointer, point.comment
        );
        seen.insert(key)
    });
}

fn severity_rank(severity: OpenApiValidationSeverity) -> u8 {
    match severity {
        OpenApiValidationSeverity::Error => 0,
        OpenApiValidationSeverity::Warning => 1,
    }
}

fn error_point(
    line: Option<u32>,
    pointer: Option<String>,
    comment: String,
) -> OpenApiValidationPoint {
    OpenApiValidationPoint {
        severity: OpenApiValidationSeverity::Error,
        line,
        pointer,
        comment,
    }
}

fn warning_point(
    line: Option<u32>,
    pointer: Option<String>,
    comment: String,
) -> OpenApiValidationPoint {
    OpenApiValidationPoint {
        severity: OpenApiValidationSeverity::Warning,
        line,
        pointer,
        comment,
    }
}

fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn unescape_pointer_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

#[cfg(test)]
mod tests {
    use crate::server::models::{OpenApiValidationSeverity, OpenApiValidationStatus};

    use super::validate_openapi_source;

    #[test]
    fn validates_minimal_valid_spec() {
        let source = r#"{
  "openapi": "3.1.0",
  "info": {
    "title": "Pet API",
    "version": "1.0.0"
  },
  "paths": {
    "/pets": {
      "get": {
        "operationId": "listPets",
        "responses": {
          "200": {
            "description": "ok"
          }
        }
      }
    }
  }
}"#;

        let result = validate_openapi_source(source);
        assert!(result.spec.is_some());
        assert!(result.points.is_empty());
        assert!(matches!(result.status, OpenApiValidationStatus::Valid));
    }

    #[test]
    fn reports_duplicated_operation_id() {
        let source = r#"{
  "openapi": "3.1.0",
  "info": {
    "title": "Pet API",
    "version": "1.0.0"
  },
  "paths": {
    "/pets": {
      "get": {
        "operationId": "sameId",
        "responses": {
          "200": {
            "description": "ok"
          }
        }
      }
    },
    "/owners": {
      "get": {
        "operationId": "sameId",
        "responses": {
          "200": {
            "description": "ok"
          }
        }
      }
    }
  }
}"#;

        let result = validate_openapi_source(source);
        assert!(matches!(result.status, OpenApiValidationStatus::Invalid));
        assert!(result.points.iter().any(|point| {
            matches!(point.severity, OpenApiValidationSeverity::Error)
                && point.comment.contains("operationId duplicado")
        }));
    }

    #[test]
    fn reports_missing_local_reference() {
        let source = r##"{
  "openapi": "3.1.0",
  "info": {
    "title": "Pet API",
    "version": "1.0.0"
  },
  "paths": {
    "/pets": {
      "get": {
        "responses": {
          "200": {
            "$ref": "#/components/responses/UnknownResponse"
          }
        }
      }
    }
  }
}"##;

        let result = validate_openapi_source(source);
        assert!(matches!(result.status, OpenApiValidationStatus::Invalid));
        assert!(result.points.iter().any(|point| {
            matches!(point.severity, OpenApiValidationSeverity::Error)
                && point.comment.contains("Referência local inválida")
        }));
    }
}
