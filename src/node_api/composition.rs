use napi::{bindgen_prelude::ClassInstance, Env, Result};
use napi_derive::napi;
use query_graph_core::{
  BatchPlanMetadata, BatchRelationDefinition, CompiledQueryPlan as CoreCompiledQueryPlan,
  ComposedQueryGraph as CoreComposedQueryGraph, MappedQueryGraph, OracleCompiler,
  SqlServerCompiler,
};
use serde_json::Value;

use crate::node_error;

use super::{
  graph::RelationalQueryGraph,
  parsing::{compile_options, parse_operation, OracleCompileOptions, SqlServerCompileOptions},
  statement::CompiledSqlStatement,
};

#[napi(object)]
pub struct CompiledBatchStep {
  pub name: String,
  pub parent_key: String,
  pub child_key: String,
  pub key_parameter: String,
  #[napi(ts_type = "Readonly<Record<string, unknown>>")]
  pub parameters: Value,
  #[napi(ts_type = "import('./dsl.js').BatchCardinality")]
  pub cardinality: String,
  pub parent_key_injected: bool,
  pub child_key_injected: bool,
}

impl From<&BatchPlanMetadata> for CompiledBatchStep {
  fn from(metadata: &BatchPlanMetadata) -> Self {
    Self {
      name: metadata.name.clone(),
      parent_key: metadata.parent_key.clone(),
      child_key: metadata.child_key.clone(),
      key_parameter: metadata.key_parameter.clone(),
      parameters: Value::Object(metadata.parameters.clone().into_iter().collect()),
      cardinality: metadata.cardinality.as_str().to_owned(),
      parent_key_injected: metadata.parent_key_injected,
      child_key_injected: metadata.child_key_injected,
    }
  }
}

#[napi]
pub struct ComposedQueryGraph {
  graph: CoreComposedQueryGraph,
}

impl ComposedQueryGraph {
  pub(super) fn new(root: MappedQueryGraph) -> Self {
    Self {
      graph: CoreComposedQueryGraph::new(root),
    }
  }
}

#[napi]
impl ComposedQueryGraph {
  #[napi(getter)]
  pub fn name(&self) -> String {
    self.graph.root().graph().definition().name.clone()
  }

  #[napi(
    ts_args_type = "graph: RelationalQueryGraph, relation: import('./dsl.js').BatchRelationWire"
  )]
  pub fn with_batch_relation(
    &self,
    env: Env,
    graph: ClassInstance<'_, RelationalQueryGraph>,
    relation: Value,
  ) -> Result<Self> {
    let relation: BatchRelationDefinition = serde_json::from_value(relation)
      .map_err(|error| node_error::composition_wire(&env, error))?;
    let graph = self
      .graph
      .clone()
      .with_batch_relation(graph.graph.clone(), relation)
      .map_err(|issues| node_error::composition(&env, &issues))?;
    Ok(Self { graph })
  }

  #[napi(
    ts_args_type = "operation: import('./dsl.js').QueryOperation, options?: import('./dsl.js').SqlServerCompileOptions"
  )]
  pub fn compile_sql_server_plan(
    &self,
    env: Env,
    operation: Value,
    options: Option<Value>,
  ) -> Result<CompiledQueryPlan> {
    let operation = parse_operation(&env, operation)?;
    let options: SqlServerCompileOptions = compile_options(&env, options)?;
    let compiler = SqlServerCompiler::new(options.version);
    let plan = self
      .graph
      .compile_sql_server_plan_with(&operation, &compiler)
      .map_err(|error| node_error::composed_compile(&env, &error))?;
    Ok(CompiledQueryPlan { plan })
  }

  #[napi(
    ts_args_type = "operation: import('./dsl.js').QueryOperation, options?: import('./dsl.js').OracleCompileOptions"
  )]
  pub fn compile_oracle_plan(
    &self,
    env: Env,
    operation: Value,
    options: Option<Value>,
  ) -> Result<CompiledQueryPlan> {
    let operation = parse_operation(&env, operation)?;
    let options: OracleCompileOptions = compile_options(&env, options)?;
    let compiler = OracleCompiler::new(options.version);
    let plan = self
      .graph
      .compile_oracle_plan_with(&operation, &compiler)
      .map_err(|error| node_error::composed_compile(&env, &error))?;
    Ok(CompiledQueryPlan { plan })
  }
}

#[napi]
pub struct CompiledQueryPlan {
  plan: CoreCompiledQueryPlan,
}

#[napi]
impl CompiledQueryPlan {
  #[napi(getter)]
  pub fn root(&self) -> CompiledSqlStatement {
    self.plan.root().clone().into()
  }

  #[napi(getter)]
  pub fn batches(&self) -> Vec<CompiledBatchStep> {
    self.plan.batches().map(Into::into).collect()
  }

  #[napi(ts_args_type = "name: string, keys: readonly unknown[]")]
  pub fn compile_batch(&self, env: Env, name: String, keys: Value) -> Result<CompiledSqlStatement> {
    let keys: Vec<Value> =
      serde_json::from_value(keys).map_err(|error| node_error::batch_keys_wire(&env, error))?;
    let statement = self
      .plan
      .compile_batch(&name, &keys)
      .map_err(|error| node_error::composed_compile(&env, &error))?;
    Ok(statement.into())
  }
}
