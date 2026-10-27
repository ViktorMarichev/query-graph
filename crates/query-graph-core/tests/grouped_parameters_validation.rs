use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition,
  LiteralValue, MappedQueryGraph, OperationIssueCode, OrderByDefinition, OrderingDefinition,
  ParameterDefinition, ParameterShape, PlanError, ProjectionFieldDefinition,
  ProjectionObjectDefinition, QueryOperation, RelationDefinition, RelationalMapping, ScalarType,
  SemanticFunction, SourceDefinition, SourceMapping, SqlCompileError,
};
use serde_json::{json, Value};

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("groupedValidation", "root");
  definition.sources.push(source("root"));
  definition
    .parameters
    .push(ParameterDefinition::optional_grouped_list(
      "filters",
      ScalarType::String,
      ScalarType::Int64,
    ));
  definition.projection.fields.push(
    ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("root", "id"))
      .selected_by_default(),
  );
  definition
}

fn source(name: &str) -> SourceDefinition {
  SourceDefinition::new(
    name,
    vec![
      FieldDefinition::new("id", ScalarType::Int64),
      FieldDefinition::new("code", ScalarType::String),
      FieldDefinition::new("active", ScalarType::Boolean),
    ],
  )
}

fn tree_definition() -> GraphDefinition {
  let mut definition = definition();
  for (from, to) in [("root", "items"), ("items", "details"), ("root", "other")] {
    definition.sources.push(source(to));
    definition.relations.push(
      RelationDefinition::new(
        to,
        from,
        to,
        Expression::eq(Expression::field(from, "id"), Expression::field(to, "id")),
      )
      .many(),
    );
  }
  definition
}

fn truth() -> Expression {
  Expression::literal(LiteralValue::Boolean(true))
}

fn template(scope: &str) -> Expression {
  Expression::all_groups("filters", scope, group_predicate("root", scope))
}

fn group_predicate(source: &str, scope: &str) -> Expression {
  Expression::and([
    Expression::eq(
      Expression::field(source, "code"),
      Expression::group_key(scope),
    ),
    Expression::in_group_values(Expression::field(source, "id"), scope),
  ])
}

fn with_predicate(mut definition: GraphDefinition, predicate: Expression) -> GraphDefinition {
  definition
    .constraints
    .push(ConstraintDefinition::always(predicate));
  definition
}

fn assert_issue(definition: GraphDefinition, code: DefinitionIssueCode, location: &str) {
  let issues = definition.compile().unwrap_err();
  assert!(
    issues
      .as_slice()
      .iter()
      .any(|issue| issue.code == code && issue.location == location),
    "expected {code:?} at {location}, got {issues}"
  );
}

fn mapped(definition: GraphDefinition) -> MappedQueryGraph {
  let sources = definition
    .sources
    .iter()
    .map(|source| (source.key.clone(), SourceMapping::new(source.key.clone())))
    .collect();
  MappedQueryGraph::new(definition.compile().unwrap(), RelationalMapping { sources }).unwrap()
}

fn operation(value: Value) -> QueryOperation {
  QueryOperation {
    parameters: HashMap::from([("filters".into(), value)]),
    ..QueryOperation::default()
  }
}

fn assert_operation_paths(graph: &MappedQueryGraph, operation: &QueryOperation, paths: &[&str]) {
  let error = graph.compile_sql_server(operation).unwrap_err();
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected operation issues, got {error:?}");
  };
  assert!(issues
    .as_slice()
    .iter()
    .all(|issue| issue.code == OperationIssueCode::InvalidParameterType));
  let mut actual: Vec<_> = issues
    .as_slice()
    .iter()
    .map(|issue| issue.location.as_str())
    .collect();
  actual.sort_unstable();
  let mut expected = paths.to_vec();
  expected.sort_unstable();
  assert_eq!(actual, expected);
}

#[test]
fn accepts_empty_groups_empty_values_repeated_keys_and_preserves_input() {
  let graph = mapped(definition());
  for value in [
    json!([]),
    json!([
      {"key": "same", "values": [1, "9223372036854775807", 1]},
      {"key": "same", "values": []},
      {"key": "", "values": [2]}
    ]),
  ] {
    let operation = operation(value);
    let before = operation.clone();
    graph.compile_sql_server(&operation).unwrap();
    assert_eq!(operation, before);
  }
}

#[test]
fn rejects_invalid_group_containers_and_members_at_exact_paths() {
  let graph = mapped(definition());
  let cases = [
    (json!(null), vec!["parameters.filters"]),
    (json!(false), vec!["parameters.filters"]),
    (
      json!({"key": "a", "values": []}),
      vec!["parameters.filters"],
    ),
    (json!([null]), vec!["parameters.filters[0]"]),
    (json!([[]]), vec!["parameters.filters[0]"]),
    (json!(["group"]), vec!["parameters.filters[0]"]),
    (json!([1]), vec!["parameters.filters[0]"]),
    (
      json!([{}]),
      vec!["parameters.filters[0].key", "parameters.filters[0].values"],
    ),
    (json!([{"key": "a"}]), vec!["parameters.filters[0].values"]),
    (json!([{"values": []}]), vec!["parameters.filters[0].key"]),
    (
      json!([{"key": "a", "values": null}]),
      vec!["parameters.filters[0].values"],
    ),
    (
      json!([{"key": "a", "values": 1}]),
      vec!["parameters.filters[0].values"],
    ),
    (
      json!([{"key": "a", "values": {}}]),
      vec!["parameters.filters[0].values"],
    ),
    (
      json!([{"key": "a", "values": "1"}]),
      vec!["parameters.filters[0].values"],
    ),
    (
      json!([{"key": null, "values": []}]),
      vec!["parameters.filters[0].key"],
    ),
    (
      json!([{"key": "a", "values": [], "extra": null}]),
      vec!["parameters.filters[0].extra"],
    ),
    (
      json!([
        {"key": "ok", "values": [1]},
        {"key": 2, "values": [null, "bad", 1.5, {}, []], "extra": true}
      ]),
      vec![
        "parameters.filters[1].key",
        "parameters.filters[1].values[0]",
        "parameters.filters[1].values[1]",
        "parameters.filters[1].values[2]",
        "parameters.filters[1].values[3]",
        "parameters.filters[1].values[4]",
        "parameters.filters[1].extra",
      ],
    ),
  ];
  for (value, paths) in cases {
    assert_operation_paths(&graph, &operation(value), &paths);
  }
}

#[test]
fn grouped_keys_and_values_share_the_existing_scalar_contract() {
  let cases = [
    (ScalarType::Boolean, json!(true), json!(1)),
    (
      ScalarType::Int32,
      json!(i32::MIN),
      json!(i64::from(i32::MIN) - 1),
    ),
    (
      ScalarType::Int32,
      json!(i32::MAX),
      json!(i64::from(i32::MAX) + 1),
    ),
    (
      ScalarType::Int64,
      json!(i64::MIN.to_string()),
      json!("-9223372036854775809"),
    ),
    (
      ScalarType::Int64,
      json!(i64::MAX.to_string()),
      json!("9223372036854775808"),
    ),
    (
      ScalarType::Int64,
      json!(9_007_199_254_740_991.0),
      json!(9_007_199_254_740_992.0),
    ),
    (ScalarType::Int64, json!(42), json!(1.5)),
    (ScalarType::Float64, json!(1.5), json!("1.5")),
    (
      ScalarType::Decimal,
      json!("1234567890.123456789"),
      json!("1e3"),
    ),
    (ScalarType::Decimal, json!(1.5), json!({})),
    (ScalarType::String, json!(""), json!(false)),
    (ScalarType::Date, json!("2026-10-27"), json!({"year": 2026})),
    (
      ScalarType::DateTime,
      json!("2026-10-27T12:00:00+06:00"),
      json!(1),
    ),
    (ScalarType::Binary, json!("base64-token"), json!([1, 2])),
  ];
  for (scalar_type, valid, invalid) in cases {
    let mut definition = definition();
    definition.parameters[0].key_type = Some(scalar_type);
    definition.parameters[0].scalar_type = scalar_type;
    let graph = mapped(definition);
    graph
      .compile_sql_server(&operation(json!([{"key": valid, "values": [valid]}])))
      .unwrap();
    assert_operation_paths(
      &graph,
      &operation(json!([{"key": invalid, "values": [valid, invalid]}])),
      &[
        "parameters.filters[0].key",
        "parameters.filters[0].values[1]",
      ],
    );
    assert_operation_paths(
      &graph,
      &operation(json!([{"key": null, "values": [null]}])),
      &[
        "parameters.filters[0].key",
        "parameters.filters[0].values[0]",
      ],
    );
  }
}

#[test]
fn json_keys_and_values_allow_null_but_never_null_containers_or_missing_members() {
  let mut definition = definition();
  definition.parameters[0].key_type = Some(ScalarType::Json);
  definition.parameters[0].scalar_type = ScalarType::Json;
  let graph = mapped(definition);
  for value in [
    json!(null),
    json!(true),
    json!(1),
    json!("x"),
    json!([]),
    json!({"nested": null}),
  ] {
    graph
      .compile_sql_server(&operation(json!([{"key": value, "values": [value]}])))
      .unwrap();
  }
  for (value, paths) in [
    (json!(null), vec!["parameters.filters"]),
    (json!([null]), vec!["parameters.filters[0]"]),
    (
      json!([{"key": null, "values": null}]),
      vec!["parameters.filters[0].values"],
    ),
    (
      json!([{}]),
      vec!["parameters.filters[0].key", "parameters.filters[0].values"],
    ),
  ] {
    assert_operation_paths(&graph, &operation(value), &paths);
  }
}

#[test]
fn allows_absent_optional_parameters_and_rejects_absent_required_parameters() {
  mapped(definition())
    .compile_sql_server(&QueryOperation::default())
    .unwrap();
  let optional = with_predicate(definition(), template("g"));
  mapped(optional)
    .compile_sql_server(&QueryOperation::default())
    .unwrap();
  let mut required = definition();
  required.parameters[0].required = true;
  let error = mapped(required)
    .compile_sql_server(&QueryOperation::default())
    .unwrap_err();
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected missing parameter issue");
  };
  assert_eq!(issues.as_slice().len(), 1);
  assert_eq!(
    issues.as_slice()[0].code,
    OperationIssueCode::MissingParameter
  );
  assert_eq!(issues.as_slice()[0].location, "parameters.filters");
}

#[test]
fn key_type_is_required_only_for_grouped_list_parameters() {
  for (shape, key_type) in [
    (ParameterShape::GroupedList, None),
    (ParameterShape::Scalar, Some(ScalarType::String)),
    (ParameterShape::List, Some(ScalarType::String)),
  ] {
    let mut definition = definition();
    definition.parameters[0].shape = shape;
    definition.parameters[0].key_type = key_type;
    assert_issue(
      definition,
      DefinitionIssueCode::InvalidParameterKeyType,
      "parameters[0].keyType",
    );
  }
  for shape in [ParameterShape::Scalar, ParameterShape::List] {
    let parameter: ParameterDefinition = serde_json::from_value(json!({
      "name": "filters", "scalarType": "int64", "shape": shape
    }))
    .unwrap();
    assert_eq!(parameter.key_type, None);
    let mut definition = definition();
    definition.parameters[0] = parameter;
    definition.compile().unwrap();
  }
}

#[test]
fn all_groups_requires_a_known_grouped_parameter() {
  assert_issue(
    with_predicate(
      definition(),
      Expression::all_groups("missing", "g", truth()),
    ),
    DefinitionIssueCode::UnknownParameter,
    "constraints[0].predicate.parameter",
  );
  for shape in [ParameterShape::Scalar, ParameterShape::List] {
    let mut definition = definition();
    definition.parameters[0].shape = shape;
    definition.parameters[0].key_type = None;
    assert_issue(
      with_predicate(definition, template("g")),
      DefinitionIssueCode::InvalidParameterShape,
      "constraints[0].predicate.parameter",
    );
  }
  for (expression, location) in [
    (Expression::parameter("filters"), "constraints[0].predicate"),
    (
      Expression::in_parameter(Expression::field("root", "id"), "filters"),
      "constraints[0].predicate.parameter",
    ),
  ] {
    assert_issue(
      with_predicate(definition(), expression),
      DefinitionIssueCode::InvalidParameterShape,
      location,
    );
  }
}

#[test]
fn accepts_nested_boolean_and_exists_templates_without_changing_the_definition() {
  let predicate = Expression::or([
    Expression::Not {
      expression: Box::new(template("g")),
    },
    Expression::exists_where(
      "items",
      Expression::all_groups(
        "filters",
        "g",
        Expression::and([
          group_predicate("items", "g"),
          Expression::exists_from_where("details", "items", group_predicate("details", "g")),
        ]),
      ),
    ),
    Expression::all_groups(
      "filters",
      "g",
      Expression::exists_where("items", group_predicate("items", "g")),
    ),
  ]);
  let definition = with_predicate(tree_definition(), predicate);
  let before = definition.clone();
  let graph = definition.compile().unwrap();
  assert_eq!(graph.definition(), &before);
}

#[test]
fn nested_templates_capture_outer_bindings_and_use_each_parameters_own_types() {
  let mut definition = tree_definition();
  definition
    .parameters
    .push(ParameterDefinition::optional_grouped_list(
      "otherFilters",
      ScalarType::Int64,
      ScalarType::String,
    ));
  let inner = Expression::all_groups(
    "otherFilters",
    "inner",
    Expression::exists_where(
      "items",
      Expression::and([
        group_predicate("items", "outer"),
        Expression::eq(
          Expression::field("items", "id"),
          Expression::group_key("inner"),
        ),
        Expression::in_group_values(Expression::field("items", "code"), "inner"),
      ]),
    ),
  );
  with_predicate(
    definition,
    Expression::all_groups(
      "filters",
      "outer",
      Expression::and([inner, group_predicate("root", "outer")]),
    ),
  )
  .compile()
  .unwrap();
}

#[test]
fn empty_and_whitespace_group_scopes_are_rejected_on_binders_and_refs() {
  for scope in ["", " \t\n"] {
    for predicate in [
      Expression::all_groups("filters", scope, truth()),
      Expression::group_key(scope),
      Expression::in_group_values(Expression::field("root", "id"), scope),
    ] {
      assert_issue(
        with_predicate(definition(), predicate),
        DefinitionIssueCode::EmptyGroupScope,
        "constraints[0].predicate.scope",
      );
    }
  }
}

#[test]
fn scope_names_are_lexical_and_do_not_resolve_parameter_names() {
  for scope in ["g", "filters", "G"] {
    for reference in [
      Expression::group_key(scope),
      Expression::in_group_values(truth(), scope),
    ] {
      assert_issue(
        with_predicate(definition(), reference),
        DefinitionIssueCode::UnknownGroupScope,
        "constraints[0].predicate.scope",
      );
    }
  }
  assert_issue(
    with_predicate(
      definition(),
      Expression::all_groups("filters", "g", Expression::group_key("G")),
    ),
    DefinitionIssueCode::UnknownGroupScope,
    "constraints[0].predicate.predicate.scope",
  );
}

#[test]
fn rejects_active_scope_shadowing_including_through_exists() {
  for predicate in [
    Expression::all_groups("filters", "g", template("g")),
    Expression::all_groups(
      "filters",
      "g",
      Expression::exists_where("items", template("g")),
    ),
  ] {
    let location = if matches!(&predicate, Expression::AllGroups { predicate, .. } if matches!(predicate.as_ref(), Expression::Exists { .. }))
    {
      "constraints[0].predicate.predicate.predicate.scope"
    } else {
      "constraints[0].predicate.predicate.scope"
    };
    assert_issue(
      with_predicate(tree_definition(), predicate),
      DefinitionIssueCode::ShadowedGroupScope,
      location,
    );
  }
}

#[test]
fn closed_binders_do_not_leak_to_siblings_or_other_constraints() {
  for reference in [
    Expression::group_key("g"),
    Expression::in_group_values(truth(), "g"),
  ] {
    assert_issue(
      with_predicate(
        definition(),
        Expression::and([template("g"), reference.clone()]),
      ),
      DefinitionIssueCode::UnknownGroupScope,
      "constraints[0].predicate.expressions[1].scope",
    );
    let definition = with_predicate(with_predicate(definition(), template("g")), reference);
    assert_issue(
      definition,
      DefinitionIssueCode::UnknownGroupScope,
      "constraints[1].predicate.scope",
    );
  }
  assert_issue(
    with_predicate(
      definition(),
      Expression::all_groups(
        "filters",
        "outer",
        Expression::and([template("inner"), Expression::group_key("inner")]),
      ),
    ),
    DefinitionIssueCode::UnknownGroupScope,
    "constraints[0].predicate.predicate.expressions[1].scope",
  );
  with_predicate(
    with_predicate(
      definition(),
      Expression::and([template("g"), template("g")]),
    ),
    template("g"),
  )
  .compile()
  .unwrap();
}

#[test]
fn refs_cannot_escape_into_relations_projection_presence_or_ordering() {
  for reference in [
    Expression::group_key("g"),
    Expression::in_group_values(truth(), "g"),
  ] {
    for target in 0..5 {
      let mut definition = with_predicate(tree_definition(), template("g"));
      let location = match target {
        0 => {
          definition.relations[0].on = reference.clone();
          "relations[0].on.scope"
        }
        1 => {
          definition.projection.fields[0].expression = reference.clone();
          "projection.fields[0].expression.scope"
        }
        2 => {
          definition.orderings.push(OrderingDefinition::new(
            "order",
            [OrderByDefinition::asc(reference.clone())],
          ));
          "orderings[0].orderBy[0].expression.scope"
        }
        3 => {
          definition.projection.fields[0].path = vec!["object".into(), "id".into()];
          definition
            .projection
            .objects
            .push(ProjectionObjectDefinition::new(
              vec!["object".into()],
              reference.clone(),
            ));
          "projection.objects[0].presence.scope"
        }
        _ => {
          definition.relations[0] = RelationDefinition::new("items", "root", "items", truth())
            .first_by([OrderByDefinition::asc(reference.clone())]);
          "relations[0].selection.orderBy[0].expression.scope"
        }
      };
      assert_issue(definition, DefinitionIssueCode::UnknownGroupScope, location);
    }
  }
}

#[test]
fn templates_are_rejected_in_non_constraint_expression_contexts() {
  for target in 0..6 {
    let mut definition = tree_definition();
    let expression = template("g");
    let location = match target {
      0 => {
        definition.projection.fields[0].expression = expression;
        "projection.fields[0].expression"
      }
      1 => {
        definition.orderings.push(OrderingDefinition::new(
          "order",
          [OrderByDefinition::asc(expression)],
        ));
        "orderings[0].orderBy[0].expression"
      }
      2 => {
        definition.relations[0].on = expression;
        "relations[0].on"
      }
      3 => {
        definition.relations[0] = RelationDefinition::new("items", "root", "items", truth())
          .first_by([OrderByDefinition::asc(expression)]);
        "relations[0].selection.orderBy[0].expression"
      }
      4 => {
        definition.projection.fields[0].path = vec!["object".into(), "id".into()];
        definition
          .projection
          .objects
          .push(ProjectionObjectDefinition::new(
            vec!["object".into()],
            expression,
          ));
        "projection.objects[0].presence"
      }
      _ => {
        definition.projection.fields[0].expression = Expression::count_of(expression);
        definition.projection.fields[0] = definition.projection.fields[0].clone().measure();
        "projection.fields[0].expression.expression"
      }
    };
    assert_issue(
      definition,
      DefinitionIssueCode::InvalidAllGroupsContext,
      location,
    );
  }
}

#[test]
fn templates_cannot_be_smuggled_into_scalar_operands() {
  for (expression, path) in [
    (Expression::eq(template("g"), truth()), "left"),
    (
      Expression::IsNull {
        expression: Box::new(template("g")),
      },
      "expression",
    ),
    (
      Expression::Function {
        name: SemanticFunction::Coalesce,
        arguments: vec![template("g"), truth()],
      },
      "arguments[0]",
    ),
    (
      Expression::In {
        expression: Box::new(truth()),
        values: vec![template("g")],
      },
      "values[0]",
    ),
    (
      Expression::in_parameter(template("g"), "flags"),
      "expression",
    ),
    (Expression::count_of(template("g")), "expression"),
  ] {
    let mut definition = definition();
    definition
      .parameters
      .push(ParameterDefinition::optional_list(
        "flags",
        ScalarType::Boolean,
      ));
    assert_issue(
      with_predicate(definition, expression),
      DefinitionIssueCode::InvalidAllGroupsContext,
      &format!("constraints[0].predicate.{path}"),
    );
  }
}

#[test]
fn all_groups_preserves_existing_exists_source_and_context_restrictions() {
  for (predicate, code, path) in [
    (
      Expression::exists_where("missing", truth()),
      DefinitionIssueCode::UnknownExistsSource,
      "source",
    ),
    (
      Expression::exists_where("root", truth()),
      DefinitionIssueCode::InvalidExistsSource,
      "source",
    ),
    (
      Expression::exists_from_where("items", "other", truth()),
      DefinitionIssueCode::InvalidExistsSource,
      "from",
    ),
    (
      Expression::exists_where("items", group_predicate("other", "g")),
      DefinitionIssueCode::ExistsExpressionScope,
      "predicate.expressions[0].left",
    ),
    (
      Expression::exists_where(
        "items",
        Expression::exists_from_where("other", "other", truth()),
      ),
      DefinitionIssueCode::InvalidExistsSource,
      "predicate.from",
    ),
  ] {
    assert_issue(
      with_predicate(
        tree_definition(),
        Expression::all_groups("filters", "g", predicate),
      ),
      code,
      &format!("constraints[0].predicate.predicate.{path}"),
    );
  }
  let mut definition = tree_definition();
  definition.projection.fields[0].expression =
    Expression::all_groups("filters", "g", Expression::exists("items"));
  assert_issue(
    definition,
    DefinitionIssueCode::InvalidExistsContext,
    "projection.fields[0].expression.predicate",
  );
  let mut definition = tree_definition();
  definition.relations[0].on =
    Expression::all_groups("filters", "g", Expression::exists("details"));
  assert_issue(
    definition,
    DefinitionIssueCode::InvalidExistsSource,
    "relations[0].on.predicate.from",
  );
}

#[test]
fn group_key_and_values_are_type_checked_against_their_own_declared_types() {
  for predicate in [
    Expression::eq(Expression::field("root", "id"), Expression::group_key("g")),
    Expression::in_group_values(Expression::field("root", "code"), "g"),
  ] {
    assert_issue(
      with_predicate(
        definition(),
        Expression::all_groups("filters", "g", predicate),
      ),
      DefinitionIssueCode::IncompatibleExpressionTypes,
      "constraints[0].predicate.predicate",
    );
  }
  for predicate in [Expression::group_key("g"), Expression::field("root", "id")] {
    assert_issue(
      with_predicate(
        definition(),
        Expression::all_groups("filters", "g", predicate),
      ),
      DefinitionIssueCode::InvalidPredicateType,
      "constraints[0].predicate.predicate",
    );
  }
  let mut definition = definition();
  definition.parameters[0].key_type = Some(ScalarType::Boolean);
  with_predicate(
    definition,
    Expression::all_groups("filters", "g", Expression::group_key("g")),
  )
  .compile()
  .unwrap();
}

#[test]
fn captured_scope_types_remain_outer_types_inside_nested_templates_and_exists() {
  for predicate in [
    Expression::eq(
      Expression::field("items", "id"),
      Expression::group_key("outer"),
    ),
    Expression::in_group_values(Expression::field("items", "code"), "outer"),
  ] {
    let mut definition = tree_definition();
    definition
      .parameters
      .push(ParameterDefinition::optional_grouped_list(
        "otherFilters",
        ScalarType::Int64,
        ScalarType::String,
      ));
    assert_issue(
      with_predicate(
        definition,
        Expression::all_groups(
          "filters",
          "outer",
          Expression::all_groups(
            "otherFilters",
            "inner",
            Expression::exists_where("items", predicate),
          ),
        ),
      ),
      DefinitionIssueCode::IncompatibleExpressionTypes,
      "constraints[0].predicate.predicate.predicate.predicate",
    );
  }
}

#[test]
fn json_group_types_keep_existing_equality_and_membership_restrictions() {
  for key in [true, false] {
    let mut definition = definition();
    let predicate = if key {
      definition.parameters[0].key_type = Some(ScalarType::Json);
      Expression::eq(Expression::group_key("g"), Expression::group_key("g"))
    } else {
      definition.parameters[0].scalar_type = ScalarType::Json;
      definition.sources[0]
        .fields
        .push(FieldDefinition::new("payload", ScalarType::Json));
      Expression::in_group_values(Expression::field("root", "payload"), "g")
    };
    assert_issue(
      with_predicate(
        definition,
        Expression::all_groups("filters", "g", predicate),
      ),
      if key {
        DefinitionIssueCode::InvalidExpressionType
      } else {
        DefinitionIssueCode::IncompatibleExpressionTypes
      },
      "constraints[0].predicate.predicate",
    );
  }
}

#[test]
fn rejects_aggregates_anywhere_inside_templates_even_in_summary_graphs() {
  for summary in [false, true] {
    for nested_exists in [false, true] {
      let mut definition = tree_definition();
      if summary {
        definition.projection.fields[0] =
          ProjectionFieldDefinition::new(vec!["count".into()], Expression::count()).measure();
      }
      let predicate = Expression::eq(
        Expression::count(),
        Expression::literal(LiteralValue::Integer(1)),
      );
      let (predicate, location) = if nested_exists {
        (
          Expression::exists_where("items", predicate),
          "constraints[0].predicate.predicate.predicate.left",
        )
      } else {
        (predicate, "constraints[0].predicate.predicate.left")
      };
      assert_issue(
        with_predicate(
          definition,
          Expression::all_groups("filters", "g", predicate),
        ),
        DefinitionIssueCode::InvalidAggregateContext,
        location,
      );
    }
  }
}

#[test]
fn rejects_templates_mixed_with_aggregate_siblings_in_having_but_allows_separate_constraints() {
  let mut definition = definition();
  definition.projection.fields[0] =
    ProjectionFieldDefinition::new(vec!["count".into()], Expression::count()).measure();
  let aggregate = Expression::eq(
    Expression::count(),
    Expression::literal(LiteralValue::Integer(1)),
  );
  for (predicate, location) in [
    (
      Expression::and([template("g"), aggregate.clone()]),
      "constraints[0].predicate.expressions[0]",
    ),
    (
      Expression::or([
        aggregate.clone(),
        Expression::Not {
          expression: Box::new(template("g")),
        },
      ]),
      "constraints[0].predicate.expressions[1].expression",
    ),
  ] {
    assert_issue(
      with_predicate(definition.clone(), predicate),
      DefinitionIssueCode::InvalidAllGroupsContext,
      location,
    );
  }
  with_predicate(with_predicate(definition, template("g")), aggregate)
    .compile()
    .unwrap();
}
