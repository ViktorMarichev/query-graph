use std::collections::{BTreeSet, HashMap};

use query_graph_core::{
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, MappedQueryGraph,
  OperationIssueCode, OrderByDefinition, OrderingDefinition, ParameterDefinition, PlanError,
  ProjectionFieldDefinition, QueryOperation, RelationDefinition, RelationalMapping, ScalarType,
  SemanticFunction, SourceDefinition, SourceMapping, SqlCompileError, SqlStatement,
};
use serde_json::{json, Value};

fn definition(predicate: Expression) -> GraphDefinition {
  let mut definition = GraphDefinition::new("groupedDependencies", "root");
  for name in ["root", "detail", "other"] {
    definition.sources.push(SourceDefinition::new(
      name,
      vec![FieldDefinition::new("id", ScalarType::Int64)],
    ));
  }
  for name in ["detail", "other"] {
    definition.relations.push(
      RelationDefinition::new(
        name,
        "root",
        name,
        Expression::eq(
          Expression::field("root", "id"),
          Expression::field(name, "id"),
        ),
      )
      .required()
      .many(),
    );
  }
  for name in ["filters", "innerFilters"] {
    definition
      .parameters
      .push(ParameterDefinition::optional_grouped_list(
        name,
        ScalarType::Int32,
        ScalarType::Int64,
      ));
  }
  for name in [
    "probe",
    "outerProbe",
    "innerProbe",
    "afterProbe",
    "joinProbe",
  ] {
    definition
      .parameters
      .push(ParameterDefinition::optional(name, ScalarType::Int64));
  }
  definition
    .constraints
    .push(ConstraintDefinition::always(predicate));
  definition.projection.fields.push(
    ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("root", "id"))
      .selected_by_default(),
  );
  definition.orderings.push(
    OrderingDefinition::new(
      "id",
      [OrderByDefinition::asc(Expression::field("root", "id"))],
    )
    .selected_by_default(),
  );
  definition
}

fn mapped(definition: GraphDefinition) -> MappedQueryGraph {
  let sources = definition
    .sources
    .iter()
    .map(|source| (source.key.clone(), SourceMapping::new(source.key.clone())))
    .collect();
  MappedQueryGraph::new(definition.compile().unwrap(), RelationalMapping { sources }).unwrap()
}

fn not_in_group(expression: Expression, scope: &str) -> Expression {
  Expression::Not {
    expression: Box::new(Expression::in_group_values(expression, scope)),
  }
}

fn operand(source: &str, parameter: &str) -> Expression {
  Expression::Function {
    name: SemanticFunction::Coalesce,
    arguments: vec![
      Expression::field(source, "id"),
      Expression::parameter(parameter),
    ],
  }
}

fn operation(groups: Value) -> QueryOperation {
  QueryOperation {
    parameters: HashMap::from([("filters".into(), groups)]),
    ..QueryOperation::default()
  }
}

fn compile_both(
  graph: &MappedQueryGraph,
  operation: &QueryOperation,
) -> [Result<SqlStatement, SqlCompileError>; 2] {
  [
    graph.compile_sql_server(operation),
    graph.compile_oracle(operation),
  ]
}

fn assert_missing_parameters(error: SqlCompileError, expected: &[&str]) {
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected missing parameter issues, got {error:?}");
  };
  assert!(issues
    .as_slice()
    .iter()
    .all(|issue| issue.code == OperationIssueCode::MissingParameter));
  let actual: BTreeSet<_> = issues
    .as_slice()
    .iter()
    .map(|issue| issue.location.clone())
    .collect();
  let expected: BTreeSet<_> = expected
    .iter()
    .map(|name| format!("parameters.{name}"))
    .collect();
  assert_eq!(actual, expected);
}

fn assert_relations(statement: &SqlStatement, expected: &[&str]) {
  let actual: BTreeSet<_> = statement
    .relations
    .iter()
    .map(|relation| relation.name.as_str())
    .collect();
  assert_eq!(actual, expected.iter().copied().collect());
  assert_eq!(statement.relations.len(), expected.len());
  assert_eq!(statement.sql.matches("JOIN ").count(), expected.len());
}

fn assert_scalar_bindings(statement: &SqlStatement, expected: &[&str]) {
  let actual: BTreeSet<_> = statement
    .bindings
    .iter()
    .filter(|binding| binding.path.is_none())
    .map(|binding| binding.parameter.as_str())
    .collect();
  assert_eq!(actual, expected.iter().copied().collect());
}

#[test]
fn empty_group_values_remove_required_many_joins_and_allow_pagination() {
  let graph = mapped(definition(Expression::all_groups(
    "filters",
    "g",
    not_in_group(Expression::field("detail", "id"), "g"),
  )));
  for groups in [
    json!([{"key": 1, "values": []}]),
    json!([
      {"key": 1, "values": []}, {"key": 1, "values": []},
    ]),
  ] {
    for limit in [None, Some(10)] {
      let mut operation = operation(groups.clone());
      operation.limit = limit;
      operation.offset = limit.map(|_| 5);
      for statement in compile_both(&graph, &operation) {
        let statement = statement.unwrap();
        assert_relations(&statement, &[]);
        assert!(statement.bindings.is_empty());
        assert!(statement.sql.contains("1 = 0"));
        assert!(!statement.sql.contains("detail"));
      }
    }
  }
}

#[test]
fn empty_group_values_do_not_require_or_bind_an_optional_scalar_operand() {
  let graph = mapped(definition(Expression::all_groups(
    "filters",
    "g",
    not_in_group(Expression::parameter("probe"), "g"),
  )));
  for probe in [None, Some(json!(42))] {
    let mut operation = operation(json!([{"key": 1, "values": []}]));
    if let Some(probe) = probe {
      operation.parameters.insert("probe".into(), probe);
    }
    for statement in compile_both(&graph, &operation) {
      let statement = statement.unwrap();
      assert_relations(&statement, &[]);
      assert!(statement.bindings.is_empty());
    }
  }
}

#[test]
fn mixed_groups_union_operand_dependencies_regardless_of_empty_group_order() {
  let graph = mapped(definition(Expression::all_groups(
    "filters",
    "g",
    not_in_group(operand("detail", "probe"), "g"),
  )));
  for groups in [
    json!([{"key": 1, "values": []}, {"key": 1, "values": [7]}]),
    json!([{"key": 1, "values": [7]}, {"key": 1, "values": []}]),
  ] {
    let mut operation = operation(groups);
    for result in compile_both(&graph, &operation) {
      assert_missing_parameters(result.unwrap_err(), &["probe"]);
    }
    operation.parameters.insert("probe".into(), json!(7));
    for statement in compile_both(&graph, &operation) {
      let statement = statement.unwrap();
      assert_relations(&statement, &["detail"]);
      assert_scalar_bindings(&statement, &["probe"]);
      assert_eq!(statement.bindings.len(), 2);
    }
    operation.limit = Some(10);
    for result in compile_both(&graph, &operation) {
      assert!(matches!(
        result,
        Err(SqlCompileError::Plan(PlanError::PaginationThroughManyRelation { relation }))
          if relation == "detail"
      ));
    }
  }
}

#[test]
fn nested_captures_union_their_own_scopes_and_restore_outer_dependencies() {
  let predicate = Expression::all_groups(
    "filters",
    "outer",
    Expression::and([
      Expression::all_groups(
        "innerFilters",
        "inner",
        Expression::and([
          not_in_group(operand("detail", "outerProbe"), "outer"),
          not_in_group(operand("other", "innerProbe"), "inner"),
        ]),
      ),
      not_in_group(Expression::parameter("afterProbe"), "outer"),
    ]),
  );
  let graph = mapped(definition(predicate));
  let empty = json!([{"key": 1, "values": []}]);
  let mixed = json!([{"key": 1, "values": []}, {"key": 1, "values": [7]}]);
  let cases = [
    (empty.clone(), Some(empty.clone()), vec![], vec![]),
    (
      empty.clone(),
      Some(mixed.clone()),
      vec!["other"],
      vec!["innerProbe"],
    ),
    (
      mixed.clone(),
      Some(empty),
      vec!["detail"],
      vec!["outerProbe", "afterProbe"],
    ),
    (
      mixed.clone(),
      Some(mixed.clone()),
      vec!["detail", "other"],
      vec!["outerProbe", "innerProbe", "afterProbe"],
    ),
    (mixed.clone(), Some(json!([])), vec![], vec!["afterProbe"]),
    (mixed.clone(), None, vec![], vec!["afterProbe"]),
    (json!([]), Some(mixed), vec![], vec![]),
  ];
  for (groups, inner_groups, expected_relations, expected_parameters) in cases {
    let mut operation = operation(groups);
    if let Some(inner_groups) = inner_groups {
      operation
        .parameters
        .insert("innerFilters".into(), inner_groups);
    }
    if !expected_parameters.is_empty() {
      for result in compile_both(&graph, &operation) {
        assert_missing_parameters(result.unwrap_err(), &expected_parameters);
      }
    }
    for parameter in &expected_parameters {
      operation.parameters.insert((*parameter).into(), json!(7));
    }
    if expected_relations.is_empty() {
      operation.limit = Some(10);
    }
    for statement in compile_both(&graph, &operation) {
      let statement = statement.unwrap();
      assert_relations(&statement, &expected_relations);
      assert_scalar_bindings(&statement, &expected_parameters);
    }
  }
}

#[test]
fn captured_values_keep_the_outer_scope_through_exists_and_retain_path_parameters() {
  let predicate = Expression::all_groups(
    "filters",
    "outer",
    Expression::exists_where(
      "detail",
      Expression::all_groups(
        "innerFilters",
        "inner",
        not_in_group(Expression::parameter("probe"), "outer"),
      ),
    ),
  );
  let mut definition = definition(predicate);
  definition.relations[0].on = Expression::and([
    definition.relations[0].on.clone(),
    Expression::eq(
      Expression::field("detail", "id"),
      Expression::parameter("joinProbe"),
    ),
  ]);
  let graph = mapped(definition);
  for (outer_values, inner_values, uses_probe) in [
    (json!([]), json!([7]), false),
    (json!([7]), json!([]), true),
    (json!([]), json!([]), false),
    (json!([7]), json!([7]), true),
  ] {
    let mut operation = operation(json!([{"key": 1, "values": outer_values}]));
    operation.parameters.insert(
      "innerFilters".into(),
      json!([{"key": 2, "values": inner_values}]),
    );
    operation.limit = Some(10);
    let expected = if uses_probe {
      vec!["joinProbe", "probe"]
    } else {
      vec!["joinProbe"]
    };
    for result in compile_both(&graph, &operation) {
      assert_missing_parameters(result.unwrap_err(), &expected);
    }
    for parameter in &expected {
      operation.parameters.insert((*parameter).into(), json!(7));
    }
    for statement in compile_both(&graph, &operation) {
      let statement = statement.unwrap();
      assert!(statement.relations.is_empty());
      assert_eq!(statement.sql.matches("EXISTS (").count(), 1);
      assert_scalar_bindings(&statement, &expected);
    }
  }
}

#[test]
fn empty_group_membership_does_not_hide_a_parameter_used_by_another_constraint() {
  let mut definition = definition(Expression::all_groups(
    "filters",
    "g",
    not_in_group(Expression::parameter("probe"), "g"),
  ));
  definition
    .constraints
    .push(ConstraintDefinition::always(Expression::eq(
      Expression::field("root", "id"),
      Expression::parameter("probe"),
    )));
  let graph = mapped(definition);
  let mut operation = operation(json!([{"key": 1, "values": []}]));
  for result in compile_both(&graph, &operation) {
    assert_missing_parameters(result.unwrap_err(), &["probe"]);
  }
  operation.parameters.insert("probe".into(), json!(7));
  for statement in compile_both(&graph, &operation) {
    let statement = statement.unwrap();
    assert_scalar_bindings(&statement, &["probe"]);
    assert_eq!(statement.bindings.len(), 1);
  }
}

#[test]
fn empty_membership_discards_exists_operand_and_its_transitive_path_parameters() {
  let mut definition = definition(Expression::all_groups(
    "filters",
    "g",
    not_in_group(
      Expression::exists_where(
        "detail",
        Expression::eq(
          Expression::field("detail", "id"),
          Expression::parameter("probe"),
        ),
      ),
      "g",
    ),
  ));
  definition.parameters[0].scalar_type = ScalarType::Boolean;
  definition.relations[0].on = Expression::and([
    definition.relations[0].on.clone(),
    Expression::eq(
      Expression::field("detail", "id"),
      Expression::parameter("joinProbe"),
    ),
  ]);
  let graph = mapped(definition);
  let mut operation = operation(json!([{"key": 1, "values": []}]));
  operation.limit = Some(10);
  for statement in compile_both(&graph, &operation) {
    let statement = statement.unwrap();
    assert_relations(&statement, &[]);
    assert!(statement.bindings.is_empty());
    assert!(!statement.sql.contains("EXISTS"));
  }
  operation
    .parameters
    .insert("filters".into(), json!([{"key": 1, "values": [true]}]));
  for result in compile_both(&graph, &operation) {
    assert_missing_parameters(result.unwrap_err(), &["joinProbe", "probe"]);
  }
  operation.parameters.insert("probe".into(), json!(7));
  operation.parameters.insert("joinProbe".into(), json!(7));
  for statement in compile_both(&graph, &operation) {
    let statement = statement.unwrap();
    assert!(statement.relations.is_empty());
    assert!(statement.sql.contains("EXISTS"));
    assert_scalar_bindings(&statement, &["joinProbe", "probe"]);
  }
}
