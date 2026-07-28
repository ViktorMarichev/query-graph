use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition,
  MappedQueryGraph, OperationIssueCode, ParameterDefinition, PlanError, ProjectionDefinition,
  ProjectionFieldDefinition, QueryOperation, RelationalMapping, ScalarType, SourceDefinition,
  SourceMapping, SqlCompileError,
};
use serde_json::json;

fn definition(required: bool) -> GraphDefinition {
  let mut definition = GraphDefinition::new("staffByIds", "staff");
  definition.sources = vec![SourceDefinition::new(
    "staff",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  )];
  definition.parameters = vec![if required {
    ParameterDefinition::required_list("ids", ScalarType::Int64)
  } else {
    ParameterDefinition::optional_list("ids", ScalarType::Int64)
  }];
  definition.constraints = vec![if required {
    ConstraintDefinition::always(
      "ids",
      Expression::in_parameter(Expression::field("staff", "id"), "ids"),
    )
  } else {
    ConstraintDefinition::when_parameter(
      "ids",
      "ids",
      Expression::in_parameter(Expression::field("staff", "id"), "ids"),
    )
  }];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["id".into()],
      Expression::field("staff", "id"),
    )
    .selected_by_default()],
  };
  definition
}

fn graph(required: bool) -> MappedQueryGraph {
  MappedQueryGraph::new(
    definition(required).compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([("staff".into(), SourceMapping::new("Staff"))]),
    },
  )
  .unwrap()
}

#[test]
fn expands_list_parameters_to_typed_sql_server_bindings() {
  let operation = QueryOperation {
    parameters: HashMap::from([("ids".into(), json!([12, 18, 24]))]),
    ..QueryOperation::default()
  };

  let statement = graph(true).compile_sql_server(&operation).unwrap();

  assert!(statement
    .sql
    .contains("WHERE\n  ([t0].[id] IN (@p0, @p1, @p2))"));
  assert_eq!(statement.bindings.len(), 3);
  for (index, binding) in statement.bindings.iter().enumerate() {
    assert_eq!(binding.name, format!("p{index}"));
    assert_eq!(binding.parameter, "ids");
    assert_eq!(binding.scalar_type, ScalarType::Int64);
    assert_eq!(binding.index, Some(index));
  }
}

#[test]
fn expands_the_same_list_semantics_to_oracle() {
  let operation = QueryOperation {
    parameters: HashMap::from([("ids".into(), json!([12, 18]))]),
    ..QueryOperation::default()
  };

  let statement = graph(true).compile_oracle(&operation).unwrap();

  assert!(statement
    .sql
    .contains("WHERE\n  (\"t0\".\"id\" IN (:p0, :p1))"));
  assert_eq!(statement.bindings[0].index, Some(0));
  assert_eq!(statement.bindings[1].index, Some(1));
}

#[test]
fn renders_an_empty_present_list_as_false_without_bindings() {
  let operation = QueryOperation {
    parameters: HashMap::from([("ids".into(), json!([]))]),
    ..QueryOperation::default()
  };

  let statement = graph(true).compile_sql_server(&operation).unwrap();

  assert!(statement.sql.contains("WHERE\n  (1 = 0)"));
  assert!(statement.bindings.is_empty());
}

#[test]
fn omits_a_conditional_constraint_when_an_optional_list_is_absent() {
  let statement = graph(false)
    .compile_sql_server(&QueryOperation::default())
    .unwrap();

  assert!(!statement.sql.contains("WHERE"));
  assert!(statement.bindings.is_empty());
}

#[test]
fn reports_each_invalid_list_element_at_its_index() {
  let operation = QueryOperation {
    parameters: HashMap::from([("ids".into(), json!([12, "not-an-id", 24.5]))]),
    ..QueryOperation::default()
  };

  let error = graph(true).compile_sql_server(&operation).unwrap_err();
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected operation validation error");
  };

  assert_eq!(issues.as_slice().len(), 2);
  assert_eq!(
    issues.as_slice()[0].code,
    OperationIssueCode::InvalidParameterType
  );
  assert_eq!(issues.as_slice()[0].location, "parameters.ids[1]");
  assert_eq!(issues.as_slice()[1].location, "parameters.ids[2]");
}

#[test]
fn rejects_a_scalar_value_for_a_list_parameter() {
  let operation = QueryOperation {
    parameters: HashMap::from([("ids".into(), json!(12))]),
    ..QueryOperation::default()
  };

  let error = graph(true).compile_sql_server(&operation).unwrap_err();
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected operation validation error");
  };

  assert_eq!(issues.as_slice()[0].location, "parameters.ids");
  assert!(issues.as_slice()[0].message.contains("expected list"));
}

#[test]
fn reuses_bindings_when_a_list_parameter_occurs_more_than_once() {
  let mut definition = definition(true);
  let membership = Expression::in_parameter(Expression::field("staff", "id"), "ids");
  definition.constraints[0].predicate = Expression::and([membership.clone(), membership]);
  let graph = MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([("staff".into(), SourceMapping::new("Staff"))]),
    },
  )
  .unwrap();
  let operation = QueryOperation {
    parameters: HashMap::from([("ids".into(), json!([12, 18]))]),
    ..QueryOperation::default()
  };

  let statement = graph.compile_sql_server(&operation).unwrap();

  assert_eq!(statement.bindings.len(), 2);
  assert_eq!(statement.sql.matches("@p0").count(), 2);
  assert_eq!(statement.sql.matches("@p1").count(), 2);
}

#[test]
fn validates_scalar_and_list_parameter_expression_shapes() {
  let mut scalar_as_list = definition(true);
  scalar_as_list.parameters = vec![ParameterDefinition::required("ids", ScalarType::Int64)];
  let scalar_as_list_issues = scalar_as_list.compile().unwrap_err();
  assert!(scalar_as_list_issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::InvalidParameterShape));

  let mut list_as_scalar = definition(true);
  list_as_scalar.constraints[0].predicate = Expression::eq(
    Expression::field("staff", "id"),
    Expression::parameter("ids"),
  );
  let list_as_scalar_issues = list_as_scalar.compile().unwrap_err();
  assert!(list_as_scalar_issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::InvalidParameterShape));
}
