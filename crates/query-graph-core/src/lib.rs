#![deny(clippy::all)]

mod compiled_graph;
mod composition;
mod definition;
mod expression;
mod mapping;
mod operation;
mod oracle;
mod path;
mod planner;
mod relational_graph;
mod scalar;
mod sql;
mod sql_server;
mod type_system;
mod type_validation;
mod validation;

pub use compiled_graph::CompiledGraph;
pub use composition::{
  BatchPlanMetadata, BatchRelationDefinition, CompiledQueryPlan, ComposedCompileError,
  ComposedQueryGraph, CompositionIssue, CompositionIssueCode, CompositionIssues,
};
pub use definition::{
  ConstraintCondition, ConstraintDefinition, FieldDefinition, GraphDefinition, NullsOrder,
  OrderByDefinition, OrderDirection, OrderingDefinition, ParameterDefinition, ParameterShape,
  ProjectionDefinition, ProjectionFieldDefinition, ProjectionFieldRole, ProjectionObjectDefinition,
  RelationCardinality, RelationDefinition, RelationSelection, ScalarType, SourceDefinition,
  GRAPH_DEFINITION_VERSION,
};
pub use expression::{AggregateFunction, Expression, LiteralValue, SemanticFunction};
pub use mapping::{
  CompiledRelationalMapping, MappingIssue, MappingIssueCode, MappingIssues, RelationalMapping,
  SourceMapping, TableName,
};
pub use operation::{OperationIssue, OperationIssueCode, OperationIssues, QueryOperation};
pub use oracle::{OracleCompiler, OracleVersion};
use path::ProjectionPath;
pub use planner::PlanError;
pub use relational_graph::MappedQueryGraph;
pub use sql::{
  ParameterBinding, SqlColumn, SqlCompileError, SqlProjectionObject, SqlRelation, SqlStatement,
};
pub use sql_server::{SqlServerCompiler, SqlServerVersion};
pub use type_system::ExpressionType;
pub use validation::{DefinitionIssue, DefinitionIssueCode, DefinitionIssues};
