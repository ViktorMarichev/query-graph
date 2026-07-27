#![deny(clippy::all)]

mod compiled_graph;
mod definition;
mod expression;
mod validation;

use napi::{Error, Result, Status};
use napi_derive::napi;

pub use compiled_graph::CompiledGraph;
pub use definition::{
  ConstraintCondition, ConstraintDefinition, FieldDefinition, GraphDefinition, NullsOrder,
  OrderByDefinition, OrderDirection, ParameterCardinality, ParameterDefinition,
  ProjectionDefinition, ProjectionFieldDefinition, RelationCardinality, RelationDefinition,
  ScalarType, SourceDefinition, GRAPH_DEFINITION_VERSION,
};
pub use expression::{Expression, LiteralValue};
pub use validation::{DefinitionIssue, DefinitionIssueCode, DefinitionIssues};

#[napi(js_name = "QueryGraph")]
pub struct QueryGraphHandle {
  graph: CompiledGraph,
}

#[napi]
impl QueryGraphHandle {
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
}

#[napi(ts_args_type = "definition: unknown")]
pub fn register_definition(definition: serde_json::Value) -> Result<QueryGraphHandle> {
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

  Ok(QueryGraphHandle { graph })
}
