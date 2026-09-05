use napi::{Env, Result};
use query_graph_core::{
  GraphDefinition, OracleVersion, QueryOperation, SqlCompileError, SqlServerVersion, SqlStatement,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

use crate::node_error;

use super::statement::CompiledSqlStatement;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SqlServerCompileOptions {
  #[serde(default)]
  pub(super) version: SqlServerVersion,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct OracleCompileOptions {
  #[serde(default)]
  pub(super) version: OracleVersion,
}

pub(super) fn compile_options<T: DeserializeOwned + Default>(
  env: &Env,
  options: Option<Value>,
) -> Result<T> {
  options.map_or_else(
    || Ok(T::default()),
    |options| {
      serde_json::from_value(options).map_err(|error| node_error::compiler_options_wire(env, error))
    },
  )
}

pub(super) fn compile_operation(
  env: &Env,
  operation: Value,
  compile: impl FnOnce(&QueryOperation) -> std::result::Result<SqlStatement, SqlCompileError>,
) -> Result<CompiledSqlStatement> {
  let operation = parse_operation(env, operation)?;
  let statement = compile(&operation).map_err(|error| node_error::sql_compile(env, &error))?;
  Ok(statement.into())
}

pub(super) fn parse_definition(env: &Env, mut definition: Value) -> Result<GraphDefinition> {
  normalize_integer_literals(&mut definition);
  serde_json::from_value(definition).map_err(|error| node_error::definition_wire(env, error))
}

pub(super) fn parse_operation(env: &Env, mut operation: Value) -> Result<QueryOperation> {
  for name in ["offset", "limit"] {
    if let Some(value) = operation.get_mut(name) {
      if value.as_f64().is_some_and(|number| number >= 0.0) {
        normalize_safe_integer(value);
      }
    }
  }
  serde_json::from_value(operation).map_err(|error| node_error::operation_wire(env, error))
}

fn normalize_integer_literals(definition: &mut Value) {
  let mut pending = vec![definition];
  while let Some(value) = pending.pop() {
    match value {
      Value::Object(object) => {
        if object.get("kind").and_then(Value::as_str) == Some("literal") {
          if let Some(Value::Object(literal)) = object.get_mut("value") {
            if literal.get("kind").and_then(Value::as_str) == Some("integer") {
              if let Some(value) = literal.get_mut("value") {
                normalize_safe_integer(value);
              }
            }
          }
        }
        pending.extend(object.values_mut());
      }
      Value::Array(values) => pending.extend(values.iter_mut()),
      _ => {}
    }
  }
}

// napi-rs represents whole JS numbers outside its i32/u32 range as JSON floats.
// Recover integers only at integer wire fields; parameter values keep their representation.
fn normalize_safe_integer(value: &mut Value) {
  const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

  let Value::Number(number) = value else {
    return;
  };
  if !number.is_f64() {
    return;
  }
  if let Some(number) = number.as_f64() {
    if number.fract() == 0.0 && (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&number) {
      *value = Value::from(number as i64);
    }
  }
}
