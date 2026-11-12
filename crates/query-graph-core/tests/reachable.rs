use query_graph_core::{
  ConstraintDefinition, CyclePolicy, Expression, FieldDefinition, GraphDefinition, OracleCompiler,
  ParameterDefinition, ProjectionFieldDefinition, QueryOperation, ReachableExpression,
  RelationalMapping, ScalarType, SourceDefinition, SqlCompileError, SqlServerCompiler,
  TraversalSide,
};
use serde_json::json;

fn row(side: TraversalSide, field: &str) -> Expression {
  Expression::TraversalField {
    scope: "ancestors".into(),
    side,
    field: field.into(),
  }
}

fn definition() -> GraphDefinition {
  let mut input = GraphDefinition::new("visibleNodes", "nodes");
  input.sources = vec![SourceDefinition::new(
    "nodes",
    vec![
      FieldDefinition::new("id", ScalarType::Int64),
      FieldDefinition::new("parentId", ScalarType::Int64).nullable(),
    ],
  )];
  input.constraints = vec![ConstraintDefinition::always(Expression::InParameter {
    expression: Box::new(Expression::field("nodes", "id")),
    parameter: "ids".into(),
  })];
  let mut graph = GraphDefinition::new("ancestors", "roots");
  graph.sources = vec![SourceDefinition::new(
    "roots",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  )];
  graph.parameters = serde_json::from_value::<Vec<ParameterDefinition>>(json!([
    { "name": "start", "scalarType": "int64", "required": true },
    { "name": "ids", "scalarType": "int64", "shape": "list", "required": true }
  ]))
  .unwrap();
  graph.constraints = vec![ConstraintDefinition::always(Expression::Reachable(
    Box::new(ReachableExpression {
      scope: "ancestors".into(),
      input,
      key: "id".into(),
      start: Expression::eq(
        Expression::field("nodes", "id"),
        Expression::parameter("start"),
      ),
      step: Expression::eq(
        row(TraversalSide::Current, "parentId"),
        row(TraversalSide::Next, "id"),
      ),
      predicate: Expression::eq(
        row(TraversalSide::Next, "id"),
        Expression::field("roots", "id"),
      ),
      cycles: CyclePolicy::Skip,
    }),
  ))];
  graph.projection.fields =
    vec![
      ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("roots", "id"))
        .selected_by_default(),
    ];
  graph
}

fn operation(size: usize) -> QueryOperation {
  serde_json::from_value(
    json!({ "parameters": { "start": size, "ids": (1..=size).collect::<Vec<_>>() } }),
  )
  .unwrap()
}

#[test]
fn traversal_compilation_is_thread_safe_and_keeps_binding_state_operation_local() {
  let original = definition();
  let graph = original.clone().compile().unwrap();
  let mapping: RelationalMapping = serde_json::from_value(json!({
    "sources": { "roots": { "table": "Nodes" }, "nodes": { "table": "Nodes" } }
  }))
  .unwrap();
  let mapping = mapping.compile(&graph).unwrap();
  std::thread::scope(|scope| {
    let handles: Vec<_> = (1..=8)
      .map(|size| {
        let graph = &graph;
        let mapping = &mapping;
        scope.spawn(move || {
          let statement = OracleCompiler::default()
            .compile(graph, mapping, &operation(size))
            .unwrap();
          assert_eq!(statement.bindings.len(), size + 1);
          assert_eq!(statement.bindings[size].parameter, "start");
          assert!(statement.sql.contains("CONNECT BY NOCYCLE"));
          assert!(!statement.sql.contains("JOIN"));
        })
      })
      .collect();
    for handle in handles {
      handle.join().unwrap();
    }
  });
  assert_eq!(graph.definition(), &original);
  assert_eq!(
    OracleCompiler::default()
      .compile(&graph, &mapping, &operation(1))
      .unwrap()
      .bindings
      .len(),
    2
  );
}

#[test]
fn traversal_wire_round_trips_and_sql_server_reports_an_explicit_capability_error() {
  let original = definition();
  let decoded: GraphDefinition =
    serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
  assert_eq!(decoded, original);
  let graph = decoded.compile().unwrap();
  let mapping: RelationalMapping = serde_json::from_value(json!({
    "sources": { "roots": { "table": "Nodes" }, "nodes": { "table": "Nodes" } }
  }))
  .unwrap();
  let mapping = mapping.compile(&graph).unwrap();
  assert!(matches!(
    SqlServerCompiler::default().compile(&graph, &mapping, &operation(1)),
    Err(SqlCompileError::UnsupportedDialectFeature {
      feature: "reachable traversal",
      ..
    })
  ));
}
