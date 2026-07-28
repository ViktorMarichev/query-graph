use napi::{
  bindgen_prelude::{JsObjectValue, ToNapiValue},
  Env, Error, Status,
};
use query_graph_core::{
  DefinitionIssues, MappingIssues, OperationIssues, PlanError, SqlCompileError,
};
use serde_json::{json, Value};

const DEFINITION_WIRE_ERROR: &str = "QUERY_GRAPH_DEFINITION_WIRE_INVALID";
const DEFINITION_ERROR: &str = "QUERY_GRAPH_DEFINITION_INVALID";
const MAPPING_WIRE_ERROR: &str = "QUERY_GRAPH_MAPPING_WIRE_INVALID";
const MAPPING_ERROR: &str = "QUERY_GRAPH_MAPPING_INVALID";
const OPERATION_WIRE_ERROR: &str = "QUERY_GRAPH_OPERATION_WIRE_INVALID";
const OPERATION_ERROR: &str = "QUERY_GRAPH_OPERATION_INVALID";
const COMPILER_OPTIONS_WIRE_ERROR: &str = "QUERY_GRAPH_COMPILER_OPTIONS_WIRE_INVALID";
const SQL_COMPILE_ERROR: &str = "QUERY_GRAPH_SQL_COMPILE_FAILED";

pub(crate) fn definition_wire(env: &Env, error: serde_json::Error) -> Error {
  wire_error(
    env,
    DEFINITION_WIRE_ERROR,
    "definition",
    "Invalid query graph definition",
    error,
  )
}

pub(crate) fn definition(env: &Env, issues: &DefinitionIssues) -> Error {
  issue_error(
    env,
    DEFINITION_ERROR,
    "definition",
    format!("Invalid query graph definition:\n{issues}"),
    serialize_issues(issues.as_slice()),
  )
}

pub(crate) fn mapping_wire(env: &Env, error: serde_json::Error) -> Error {
  wire_error(
    env,
    MAPPING_WIRE_ERROR,
    "mapping",
    "Invalid relational mapping",
    error,
  )
}

pub(crate) fn mapping(env: &Env, issues: &MappingIssues) -> Error {
  issue_error(
    env,
    MAPPING_ERROR,
    "mapping",
    format!("Invalid relational mapping:\n{issues}"),
    serialize_issues(issues.as_slice()),
  )
}

pub(crate) fn operation_wire(env: &Env, error: serde_json::Error) -> Error {
  wire_error(
    env,
    OPERATION_WIRE_ERROR,
    "operation",
    "Invalid query operation",
    error,
  )
}

pub(crate) fn compiler_options_wire(env: &Env, error: serde_json::Error) -> Error {
  wire_error(
    env,
    COMPILER_OPTIONS_WIRE_ERROR,
    "sql",
    "Invalid SQL compiler options",
    error,
  )
}

pub(crate) fn sql_compile(env: &Env, error: &SqlCompileError) -> Error {
  if let SqlCompileError::Plan(PlanError::Operation(issues)) = error {
    return operation(env, issues);
  }

  issue_error(
    env,
    SQL_COMPILE_ERROR,
    "sql",
    format!("Unable to compile SQL: {error}"),
    json!([sql_diagnostic(error)]),
  )
}

fn operation(env: &Env, issues: &OperationIssues) -> Error {
  issue_error(
    env,
    OPERATION_ERROR,
    "operation",
    format!("Unable to compile SQL:\n{issues}"),
    serialize_issues(issues.as_slice()),
  )
}

fn wire_error(
  env: &Env,
  code: &'static str,
  phase: &'static str,
  message: &'static str,
  error: serde_json::Error,
) -> Error {
  let message = format!("{message}: {error}");
  issue_error(
    env,
    code,
    phase,
    message.clone(),
    json!([{
      "code": "invalidWireFormat",
      "location": phase,
      "message": error.to_string(),
    }]),
  )
}

fn issue_error(env: &Env, code: &str, phase: &str, message: String, issues: Value) -> Error {
  create_error(env, code, phase, &message, issues)
    .unwrap_or_else(|_| Error::new(Status::InvalidArg, message))
}

fn create_error(
  env: &Env,
  code: &str,
  phase: &str,
  message: &str,
  issues: Value,
) -> napi::Result<Error> {
  let mut error = env.create_error(Error::new(Status::InvalidArg, message.to_owned()))?;
  error.set_named_property("name", "QueryGraphError")?;
  error.set_named_property("code", code)?;
  error.set_named_property("phase", phase)?;
  error.set_named_property("issues", env.to_js_value(&issues)?)?;
  Ok(Error::from(error.into_unknown(env)?))
}

fn serialize_issues(issues: impl serde::Serialize) -> Value {
  serde_json::to_value(issues).unwrap_or_else(|error| {
    json!([{
      "code": "diagnosticSerializationFailed",
      "location": "",
      "message": error.to_string(),
    }])
  })
}

fn sql_diagnostic(error: &SqlCompileError) -> Value {
  let (code, location) = match error {
    SqlCompileError::Plan(PlanError::InvalidCompiledGraph { .. }) => {
      ("invalidCompiledGraph", "plan")
    }
    SqlCompileError::Plan(PlanError::PaginationThroughManyRelation { .. }) => {
      ("paginationThroughManyRelation", "operation")
    }
    SqlCompileError::Plan(PlanError::AggregationAcrossManyBranches { .. }) => {
      ("aggregationAcrossManyBranches", "plan")
    }
    SqlCompileError::Plan(PlanError::Operation(_)) => ("invalidOperation", "operation"),
    SqlCompileError::MissingSourceMapping(_) => ("missingSourceMapping", "mapping"),
    SqlCompileError::MissingParameter(_) => ("missingParameter", "operation.parameters"),
    SqlCompileError::UnsupportedTableQualifier { .. } => ("unsupportedTableQualifier", "mapping"),
    SqlCompileError::UnsupportedDialectFeature { .. } => ("unsupportedDialectFeature", "sql"),
    SqlCompileError::PaginationRequiresOrder { .. } => ("paginationRequiresOrder", "operation"),
  };

  json!({
    "code": code,
    "location": location,
    "message": error.to_string(),
  })
}
