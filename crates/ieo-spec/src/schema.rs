//! Format layer. Schemas are compiled from the files in `docs/poc/` so the
//! contract has a single source.

use crate::rejection::{Code, Rejection, pointer_to_field};
use jsonschema::Validator;
use jsonschema::error::ValidationErrorKind;
use serde_json::Value;
use std::sync::OnceLock;

pub const SPRINT_SPEC_SCHEMA: &str = include_str!("../../../docs/poc/SprintSpec_v0.schema.json");
pub const HUMAN_INPUT_SCHEMA: &str =
    include_str!("../../../docs/poc/HumanSprintInput_v0.schema.json");

fn compile(src: &str) -> Validator {
    let schema: Value = serde_json::from_str(src).unwrap_or(Value::Null);
    match jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
    {
        Ok(v) => v,
        // The schema files are fixed contract inputs checked by tests; a broken
        // schema must stop the program rather than accept everything.
        Err(e) => panic!("contract schema does not compile: {e}"),
    }
}

pub fn sprint_spec_validator() -> &'static Validator {
    static V: OnceLock<Validator> = OnceLock::new();
    V.get_or_init(|| compile(SPRINT_SPEC_SCHEMA))
}

pub fn human_input_validator() -> &'static Validator {
    static V: OnceLock<Validator> = OnceLock::new();
    V.get_or_init(|| compile(HUMAN_INPUT_SCHEMA))
}

/// Run a validator and turn every error into a correctable rejection.
pub fn format_rejections(validator: &Validator, instance: &Value) -> Vec<Rejection> {
    let mut out = Vec::new();
    for err in validator.iter_errors(instance) {
        let base = pointer_to_field(err.instance_path().as_str());
        let r = match err.kind() {
            ValidationErrorKind::Required { property } => {
                let name = property.as_str().unwrap_or_default().to_string();
                let field = if base.is_empty() {
                    name.clone()
                } else {
                    format!("{base}.{name}")
                };
                Rejection::new(
                    Code::MissingField,
                    field,
                    format!("필수 필드 `{name}`이(가) 없습니다."),
                    "해당 필드를 추가하세요. 값을 아직 모르면 계약이 허용하는 null/미확정 표현과 이유를 쓰세요.",
                )
            }
            _ => Rejection::new(
                Code::SchemaViolation,
                base,
                err.to_string(),
                "스키마가 허용하는 형식·값으로 고치세요.",
            ),
        };
        out.push(r);
    }
    out.sort_by(|a, b| (a.code, &a.field).cmp(&(b.code, &b.field)));
    out.dedup();
    out
}
