use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, LiteralValue,
  MappedQueryGraph, OracleCompiler, OracleVersion, OrderByDefinition, ParameterDefinition,
  ProjectionFieldDefinition, ProjectionObjectDefinition, QueryOperation, RelationDefinition,
  RelationalMapping, ScalarType, SemanticFunction, SourceDefinition, SourceMapping,
};
use serde_json::json;

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("booleans", "item");
  definition.sources = vec![SourceDefinition::new(
    "item",
    vec![
      FieldDefinition::new("id", ScalarType::Int64),
      FieldDefinition::new("score", ScalarType::Int64).nullable(),
    ],
  )];
  definition
}

fn mapped(definition: GraphDefinition) -> MappedQueryGraph {
  let sources = definition
    .sources
    .iter()
    .map(|source| {
      (
        source.key.clone(),
        SourceMapping::new(if source.key == "item" {
          "Item"
        } else {
          "Child"
        }),
      )
    })
    .collect();
  MappedQueryGraph::new(definition.compile().unwrap(), RelationalMapping { sources }).unwrap()
}

fn score_equals_one() -> Expression {
  Expression::eq(
    Expression::field("item", "score"),
    Expression::literal(LiteralValue::Integer(1)),
  )
}

#[test]
fn renders_boolean_dimensions_and_measures_as_values_and_having_as_a_condition() {
  let mut definition = definition();
  let measure = Expression::eq(
    Expression::sum(Expression::field("item", "score")),
    Expression::literal(LiteralValue::Integer(1)),
  );
  definition.projection.fields = vec![
    ProjectionFieldDefinition::new(vec!["matches".into()], score_equals_one())
      .dimension()
      .selected_by_default(),
    ProjectionFieldDefinition::new(vec!["totalMatches".into()], measure.clone())
      .measure()
      .selected_by_default(),
  ];
  definition.constraints = vec![ConstraintDefinition::always(Expression::Function {
    name: SemanticFunction::Coalesce,
    arguments: vec![measure, Expression::literal(LiteralValue::Boolean(false))],
  })];
  let graph = mapped(definition);

  for (statement, field) in [
    (
      graph
        .compile_sql_server(&QueryOperation::default())
        .unwrap(),
      "[t0].[score]",
    ),
    (
      graph.compile_oracle(&QueryOperation::default()).unwrap(),
      "\"t0\".\"score\"",
    ),
  ] {
    let dimension =
      format!("(CASE WHEN ({field} = 1) THEN 1 WHEN NOT (({field} = 1)) THEN 0 ELSE NULL END)");
    let measure = format!(
      "(CASE WHEN (SUM({field}) = 1) THEN 1 WHEN NOT ((SUM({field}) = 1)) THEN 0 ELSE NULL END)"
    );
    assert!(statement.sql.contains(&format!("  {dimension} AS ")));
    assert!(statement.sql.contains(&format!("  {measure} AS ")));
    assert!(statement
      .sql
      .contains(&format!("\nGROUP BY\n  {dimension}")));
    assert!(statement
      .sql
      .contains(&format!("\nHAVING\n  (COALESCE({measure}, 0) = 1)")));
    assert!(statement
      .columns
      .iter()
      .all(|column| column.scalar_type == ScalarType::Boolean && column.nullable));
    assert!(statement.bindings.is_empty());
  }

  let native = graph
    .compile_oracle_with(
      &QueryOperation::default(),
      &OracleCompiler::new(OracleVersion::V23ai),
    )
    .unwrap();
  assert!(native.sql.contains("  (\"t0\".\"score\" = 1) AS \"c0\""));
  assert!(native.sql.contains("\nGROUP BY\n  (\"t0\".\"score\" = 1)"));
  assert!(native
    .sql
    .contains("\nHAVING\n  TO_BOOLEAN(COALESCE((SUM(\"t0\".\"score\") = 1), 0))"));
}

#[test]
fn renders_predicate_presence_as_a_nullable_value() {
  let mut definition = definition();
  definition.projection.fields = vec![ProjectionFieldDefinition::new(
    vec!["detail".into(), "id".into()],
    Expression::field("item", "id"),
  )
  .selected_by_default()];
  definition.projection.objects = vec![ProjectionObjectDefinition::new(
    vec!["detail".into()],
    score_equals_one(),
  )];
  let graph = mapped(definition);
  let statement = graph
    .compile_sql_server(&QueryOperation::default())
    .unwrap();
  assert!(statement.sql.contains(
    "(CASE WHEN ([t0].[score] = 1) THEN 1 WHEN NOT (([t0].[score] = 1)) THEN 0 ELSE NULL END) AS [o0]"
  ));
  assert_eq!(statement.objects[0].path, "detail");
  assert_eq!(statement.objects[0].presence_column, "o0");
}

#[test]
fn renders_first_by_and_exists_conditions_without_reordering_parameters() {
  let mut definition = definition();
  definition.sources.push(SourceDefinition::new(
    "child",
    vec![
      FieldDefinition::new("idItem", ScalarType::Int64),
      FieldDefinition::new("name", ScalarType::String).nullable(),
      FieldDefinition::new("enabled", ScalarType::Boolean).nullable(),
    ],
  ));
  definition.parameters = vec![
    ParameterDefinition::required("enabled", ScalarType::Boolean),
    ParameterDefinition::required("fallback", ScalarType::String),
  ];
  definition.relations = vec![RelationDefinition::new(
    "child",
    "item",
    "child",
    Expression::and([
      Expression::eq(
        Expression::field("item", "id"),
        Expression::field("child", "idItem"),
      ),
      Expression::parameter("enabled"),
    ]),
  )
  .first_by([OrderByDefinition::asc(Expression::Function {
    name: SemanticFunction::Coalesce,
    arguments: vec![
      Expression::field("child", "name"),
      Expression::parameter("fallback"),
    ],
  })])];
  definition.projection.fields =
    vec![
      ProjectionFieldDefinition::new(vec!["name".into()], Expression::field("child", "name"))
        .selected_by_default(),
    ];
  definition.constraints = vec![ConstraintDefinition::always(Expression::exists_where(
    "child",
    Expression::field("child", "enabled"),
  ))];
  let graph = mapped(definition);
  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([
        ("enabled".into(), json!(true)),
        ("fallback".into(), json!("")),
      ]),
      ..QueryOperation::default()
    })
    .unwrap();
  assert!(statement
    .sql
    .contains("ORDER BY COALESCE([t1].[name], @p0) ASC"));
  assert!(statement
    .sql
    .contains("WHERE (([t0].[id] = [t1].[idItem]) AND (@p1 = 1))"));
  assert!(statement.sql.contains("    WHERE ([t1].[enabled] = 1)"));
  assert_eq!(
    statement
      .bindings
      .iter()
      .map(|binding| binding.parameter.as_str())
      .collect::<Vec<_>>(),
    ["fallback", "enabled"]
  );
}
