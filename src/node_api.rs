use std::sync::Arc;

use napi::{Error, Result, Status};
use napi_derive::napi;

use crate::{
  CompiledGraph, GraphDefinition, MappedQueryGraph, ParameterBinding, QueryOperation,
  RelationalMapping, SqlStatement,
};

#[napi(object)]
pub struct CompiledSqlStatement {
  pub sql: String,
  pub bindings: Vec<SqlBinding>,
  pub fields: Vec<String>,
}

impl From<SqlStatement> for CompiledSqlStatement {
  fn from(statement: SqlStatement) -> Self {
    Self {
      sql: statement.sql,
      bindings: statement
        .bindings
        .into_iter()
        .map(SqlBinding::from)
        .collect(),
      fields: statement.fields,
    }
  }
}

#[napi(object)]
pub struct SqlBinding {
  pub name: String,
  pub parameter: String,
}

impl From<ParameterBinding> for SqlBinding {
  fn from(binding: ParameterBinding) -> Self {
    Self {
      name: binding.name,
      parameter: binding.parameter,
    }
  }
}

#[napi]
pub struct QueryGraph {
  graph: Arc<CompiledGraph>,
}

#[napi]
impl QueryGraph {
  #[napi(getter)]
  pub fn name(&self) -> String {
    self.graph.definition().name.clone()
  }

  #[napi(getter)]
  pub fn root(&self) -> String {
    self.graph.root().key.clone()
  }

  #[napi(getter)]
  pub fn source_count(&self) -> u32 {
    u32::try_from(self.graph.definition().sources.len()).unwrap_or(u32::MAX)
  }

  #[napi(getter)]
  pub fn relation_count(&self) -> u32 {
    u32::try_from(self.graph.definition().relations.len()).unwrap_or(u32::MAX)
  }

  #[napi]
  pub fn has_source(&self, source: String) -> bool {
    self.graph.source(&source).is_some()
  }

  #[napi]
  pub fn has_field(&self, source: String, field: String) -> bool {
    self.graph.field(&source, &field).is_some()
  }

  #[napi]
  pub fn has_parameter(&self, parameter: String) -> bool {
    self.graph.parameter(&parameter).is_some()
  }

  #[napi]
  pub fn has_relation(&self, relation: String) -> bool {
    self.graph.relation(&relation).is_some()
  }

  #[napi]
  pub fn selectable_fields(&self) -> Vec<String> {
    self
      .graph
      .definition()
      .projection
      .fields
      .iter()
      .filter(|field| field.selectable)
      .map(|field| field.path.join("."))
      .collect()
  }

  #[napi(ts_args_type = "mapping: unknown")]
  pub fn with_relational_mapping(
    &self,
    mapping: serde_json::Value,
  ) -> Result<RelationalQueryGraph> {
    let mapping: RelationalMapping = serde_json::from_value(mapping).map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("Invalid relational mapping: {error}"),
      )
    })?;

    let graph = MappedQueryGraph::new(Arc::clone(&self.graph), mapping).map_err(|issues| {
      Error::new(
        Status::InvalidArg,
        format!("Invalid relational mapping:\n{issues}"),
      )
    })?;

    Ok(RelationalQueryGraph { graph })
  }
}

#[napi]
pub struct RelationalQueryGraph {
  graph: MappedQueryGraph,
}

#[napi]
impl RelationalQueryGraph {
  #[napi(getter)]
  pub fn name(&self) -> String {
    self.graph.graph().definition().name.clone()
  }

  #[napi(ts_args_type = "operation: unknown")]
  pub fn compile_sql_server(&self, operation: serde_json::Value) -> Result<CompiledSqlStatement> {
    let operation: QueryOperation = serde_json::from_value(operation).map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("Invalid query operation: {error}"),
      )
    })?;

    self
      .graph
      .compile_sql_server(&operation)
      .map(CompiledSqlStatement::from)
      .map_err(|error| {
        Error::new(
          Status::InvalidArg,
          format!("Unable to compile SQL: {error}"),
        )
      })
  }
}

#[napi(ts_args_type = "definition: unknown")]
pub fn register_definition(definition: serde_json::Value) -> Result<QueryGraph> {
  let definition: GraphDefinition = serde_json::from_value(definition).map_err(|error| {
    Error::new(
      Status::InvalidArg,
      format!("Invalid query graph definition: {error}"),
    )
  })?;

  let graph = definition.compile().map_err(|issues| {
    Error::new(
      Status::InvalidArg,
      format!("Invalid query graph definition:\n{issues}"),
    )
  })?;

  Ok(QueryGraph {
    graph: Arc::new(graph),
  })
}
