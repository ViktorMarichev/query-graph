use napi::{Env, Result};
use query_graph_core::{
  OracleVersion, QueryOperation, SqlCompileError, SqlServerVersion, SqlStatement,
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

pub(super) fn parse_operation(env: &Env, operation: Value) -> Result<QueryOperation> {
  serde_json::from_value(operation).map_err(|error| node_error::operation_wire(env, error))
}
