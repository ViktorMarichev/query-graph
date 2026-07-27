#![deny(clippy::all)]

mod compiled_graph;
mod definition;
mod expression;
mod mapping;
mod node_api;
mod operation;
mod path;
mod planner;
mod relational_graph;
mod sql_server;
mod validation;

pub use compiled_graph::CompiledGraph;
pub use definition::{
  ConstraintCondition, ConstraintDefinition, FieldDefinition, GraphDefinition, NullsOrder,
  OrderByDefinition, OrderDirection, ParameterCardinality, ParameterDefinition,
  ProjectionDefinition, ProjectionFieldDefinition, RelationCardinality, RelationDefinition,
  ScalarType, SourceDefinition, GRAPH_DEFINITION_VERSION,
};
pub use expression::{Expression, LiteralValue};
pub use mapping::{
  CompiledRelationalMapping, MappingIssue, MappingIssueCode, MappingIssues, RelationalMapping,
  SourceMapping, TableName,
};
pub use node_api::{
  register_definition, CompiledSqlStatement, QueryGraph, RelationalQueryGraph, SqlBinding,
};
pub use operation::{OperationIssue, OperationIssueCode, OperationIssues, QueryOperation};
use path::ProjectionPath;
pub use planner::PlanError;
pub use relational_graph::MappedQueryGraph;
pub use sql_server::{ParameterBinding, SqlCompileError, SqlStatement};
pub use validation::{DefinitionIssue, DefinitionIssueCode, DefinitionIssues};
