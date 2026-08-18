use std::sync::Arc;

use napi::{Env, Result};
use napi_derive::napi;
use query_graph_core::{
  CompiledGraph, GraphDefinition, MappedQueryGraph, OracleCompiler, RelationalMapping,
  SqlServerCompiler,
};
use serde_json::Value;

use crate::node_error;

use super::{
  composition::ComposedQueryGraph,
  parsing::{compile_operation, compile_options, OracleCompileOptions, SqlServerCompileOptions},
  statement::CompiledSqlStatement,
};

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
      .map(|field| field.path.join("."))
      .collect()
  }

  #[napi(ts_args_type = "mapping: import('./dsl.js').RelationalMapping")]
  pub fn with_relational_mapping(&self, env: Env, mapping: Value) -> Result<RelationalQueryGraph> {
    let mapping: RelationalMapping =
      serde_json::from_value(mapping).map_err(|error| node_error::mapping_wire(&env, error))?;
    let graph = MappedQueryGraph::new(Arc::clone(&self.graph), mapping)
      .map_err(|issues| node_error::mapping(&env, &issues))?;

    Ok(RelationalQueryGraph { graph })
  }

  #[napi(ts_args_type = "mappings: readonly import('./dsl.js').RelationalMapping[]")]
  pub fn with_relational_mappings(
    &self,
    env: Env,
    mappings: Vec<Value>,
  ) -> Result<RelationalQueryGraph> {
    let mappings = mappings
      .into_iter()
      .map(|mapping| {
        serde_json::from_value(mapping).map_err(|error| node_error::mapping_wire(&env, error))
      })
      .collect::<Result<Vec<RelationalMapping>>>()?;
    let mapping =
      RelationalMapping::merge(mappings).map_err(|issues| node_error::mapping(&env, &issues))?;
    let graph = MappedQueryGraph::new(Arc::clone(&self.graph), mapping)
      .map_err(|issues| node_error::mapping(&env, &issues))?;

    Ok(RelationalQueryGraph { graph })
  }
}

#[napi]
pub struct RelationalQueryGraph {
  pub(super) graph: MappedQueryGraph,
}

#[napi]
impl RelationalQueryGraph {
  #[napi(getter)]
  pub fn name(&self) -> String {
    self.graph.graph().definition().name.clone()
  }

  #[napi]
  pub fn compose(&self) -> ComposedQueryGraph {
    ComposedQueryGraph::new(self.graph.clone())
  }

  #[napi(
    ts_args_type = "operation: import('./dsl.js').QueryOperation, options?: import('./dsl.js').SqlServerCompileOptions"
  )]
  pub fn compile_sql_server(
    &self,
    env: Env,
    operation: Value,
    options: Option<Value>,
  ) -> Result<CompiledSqlStatement> {
    let options: SqlServerCompileOptions = compile_options(&env, options)?;
    let compiler = SqlServerCompiler::new(options.version);
    compile_operation(&env, operation, |operation| {
      self.graph.compile_sql_server_with(operation, &compiler)
    })
  }

  #[napi(
    ts_args_type = "operation: import('./dsl.js').QueryOperation, options?: import('./dsl.js').OracleCompileOptions"
  )]
  pub fn compile_oracle(
    &self,
    env: Env,
    operation: Value,
    options: Option<Value>,
  ) -> Result<CompiledSqlStatement> {
    let options: OracleCompileOptions = compile_options(&env, options)?;
    let compiler = OracleCompiler::new(options.version);
    compile_operation(&env, operation, |operation| {
      self.graph.compile_oracle_with(operation, &compiler)
    })
  }
}

#[napi(
  ts_generic_types = "const Definition extends import('./dsl.js').GraphDefinitionInput",
  ts_args_type = "definition: import('./dsl.js').ExactGraphDefinitionInput<Definition>",
  ts_return_type = "import('./dsl.js').QueryGraph<Definition>"
)]
pub fn register_definition(env: Env, definition: Value) -> Result<QueryGraph> {
  let definition: GraphDefinition =
    serde_json::from_value(definition).map_err(|error| node_error::definition_wire(&env, error))?;
  let graph = definition
    .compile()
    .map_err(|issues| node_error::definition(&env, &issues))?;

  Ok(QueryGraph {
    graph: Arc::new(graph),
  })
}
