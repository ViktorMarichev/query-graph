use std::sync::Arc;

use napi::{bindgen_prelude::ClassInstance, Env, Result};
use napi_derive::napi;
use query_graph_core::{
  BatchPlanMetadata, BatchRelationDefinition, CompiledGraph,
  CompiledQueryPlan as CoreCompiledQueryPlan, ComposedQueryGraph as CoreComposedQueryGraph,
  GraphDefinition, MappedQueryGraph, OracleCompiler, OracleVersion, ParameterBinding,
  QueryOperation, RelationalMapping, SqlColumn, SqlCompileError, SqlRelation, SqlServerCompiler,
  SqlServerVersion, SqlStatement,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

use crate::node_error;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SqlServerCompileOptions {
  #[serde(default)]
  version: SqlServerVersion,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleCompileOptions {
  #[serde(default)]
  version: OracleVersion,
}

#[napi(object)]
pub struct CompiledSqlStatement {
  pub sql: String,
  pub bindings: Vec<SqlBinding>,
  pub columns: Vec<CompiledSqlColumn>,
  pub relations: Vec<CompiledSqlRelation>,
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
      columns: statement.columns.into_iter().map(Into::into).collect(),
      relations: statement.relations.into_iter().map(Into::into).collect(),
    }
  }
}

#[napi(object)]
pub struct SqlBinding {
  pub name: String,
  pub parameter: String,
  #[napi(ts_type = "import('./definition.js').ScalarType")]
  pub scalar_type: String,
  pub index: Option<u32>,
}

impl From<ParameterBinding> for SqlBinding {
  fn from(binding: ParameterBinding) -> Self {
    Self {
      name: binding.name,
      parameter: binding.parameter,
      scalar_type: binding.scalar_type.as_str().to_owned(),
      index: binding
        .index
        .map(|index| u32::try_from(index).unwrap_or(u32::MAX)),
    }
  }
}

#[napi(object)]
pub struct CompiledSqlColumn {
  pub name: String,
  pub path: String,
  #[napi(ts_type = "import('./definition.js').ScalarType")]
  pub scalar_type: String,
  pub nullable: bool,
  pub relations: Vec<String>,
}

impl From<SqlColumn> for CompiledSqlColumn {
  fn from(column: SqlColumn) -> Self {
    Self {
      name: column.name,
      path: column.path,
      scalar_type: column.scalar_type.as_str().to_owned(),
      nullable: column.nullable,
      relations: column.relations,
    }
  }
}

#[napi(object)]
pub struct CompiledSqlRelation {
  pub name: String,
  pub from: String,
  pub to: String,
  #[napi(ts_type = "import('./definition.js').RelationCardinality")]
  pub cardinality: String,
  pub required: bool,
}

impl From<SqlRelation> for CompiledSqlRelation {
  fn from(relation: SqlRelation) -> Self {
    Self {
      name: relation.name,
      from: relation.from,
      to: relation.to,
      cardinality: relation.cardinality.as_str().to_owned(),
      required: relation.required,
    }
  }
}

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

  #[napi(ts_args_type = "mapping: import('./definition.js').RelationalMapping")]
  pub fn with_relational_mapping(
    &self,
    env: Env,
    mapping: serde_json::Value,
  ) -> Result<RelationalQueryGraph> {
    let mapping: RelationalMapping =
      serde_json::from_value(mapping).map_err(|error| node_error::mapping_wire(&env, error))?;

    let graph = MappedQueryGraph::new(Arc::clone(&self.graph), mapping)
      .map_err(|issues| node_error::mapping(&env, &issues))?;

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

  #[napi]
  pub fn compose(&self) -> ComposedQueryGraph {
    ComposedQueryGraph {
      graph: CoreComposedQueryGraph::new(self.graph.clone()),
    }
  }

  #[napi(
    ts_args_type = "operation: import('./definition.js').QueryOperation, options?: import('./definition.js').SqlServerCompileOptions"
  )]
  pub fn compile_sql_server(
    &self,
    env: Env,
    operation: serde_json::Value,
    options: Option<serde_json::Value>,
  ) -> Result<CompiledSqlStatement> {
    let options: SqlServerCompileOptions = compile_options(&env, options)?;
    let compiler = SqlServerCompiler::new(options.version);
    compile_operation(&env, operation, |operation| {
      self.graph.compile_sql_server_with(operation, &compiler)
    })
  }

  #[napi(
    ts_args_type = "operation: import('./definition.js').QueryOperation, options?: import('./definition.js').OracleCompileOptions"
  )]
  pub fn compile_oracle(
    &self,
    env: Env,
    operation: serde_json::Value,
    options: Option<serde_json::Value>,
  ) -> Result<CompiledSqlStatement> {
    let options: OracleCompileOptions = compile_options(&env, options)?;
    let compiler = OracleCompiler::new(options.version);
    compile_operation(&env, operation, |operation| {
      self.graph.compile_oracle_with(operation, &compiler)
    })
  }
}

#[napi]
pub struct ComposedQueryGraph {
  graph: CoreComposedQueryGraph,
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
    ts_args_type = "operation: import('./definition.js').QueryOperation, options?: import('./definition.js').SqlServerCompileOptions"
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
    ts_args_type = "operation: import('./definition.js').QueryOperation, options?: import('./definition.js').OracleCompileOptions"
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

fn compile_options<T: DeserializeOwned + Default>(
  env: &Env,
  options: Option<serde_json::Value>,
) -> Result<T> {
  options.map_or_else(
    || Ok(T::default()),
    |options| {
      serde_json::from_value(options).map_err(|error| node_error::compiler_options_wire(env, error))
    },
  )
}

fn compile_operation(
  env: &Env,
  operation: serde_json::Value,
  compile: impl FnOnce(&QueryOperation) -> std::result::Result<SqlStatement, SqlCompileError>,
) -> Result<CompiledSqlStatement> {
  let operation = parse_operation(env, operation)?;
  let statement = compile(&operation).map_err(|error| node_error::sql_compile(env, &error))?;
  Ok(statement.into())
}

fn parse_operation(env: &Env, operation: Value) -> Result<QueryOperation> {
  serde_json::from_value(operation).map_err(|error| node_error::operation_wire(env, error))
}

#[napi(
  ts_generic_types = "const Definition extends import('./definition.js').GraphDefinitionInput",
  ts_args_type = "definition: import('./definition.js').ExactGraphDefinitionInput<Definition>",
  ts_return_type = "import('./definition.js').QueryGraph<Definition>"
)]
pub fn register_definition(env: Env, definition: serde_json::Value) -> Result<QueryGraph> {
  let definition: GraphDefinition =
    serde_json::from_value(definition).map_err(|error| node_error::definition_wire(&env, error))?;
  let graph = definition
    .compile()
    .map_err(|issues| node_error::definition(&env, &issues))?;

  Ok(QueryGraph {
    graph: Arc::new(graph),
  })
}
