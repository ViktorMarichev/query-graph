use query_graph_core::{
  ConstraintCondition, ConstraintDefinition, CyclePolicy, DefinitionIssueCode, Expression,
  FieldDefinition, GraphDefinition, OracleCompiler, ParameterDefinition, ProjectionFieldDefinition,
  QueryOperation, RelationalMapping, ScalarType, SourceDefinition, SqlCompileError,
  SqlServerCompiler, TraversalDefinition, TraversalPathDefinition, TraversalSide,
};
use serde_json::json;

fn definition() -> GraphDefinition {
  let fields = vec![
    FieldDefinition::new("id", ScalarType::Int64),
    FieldDefinition::new("parentId", ScalarType::Int64).nullable(),
  ];
  let mut input = GraphDefinition::new("eligible", "nodes");
  input.sources = vec![SourceDefinition::new("nodes", fields.clone())];
  let row = |side, field: &str| Expression::TraversalField {
    scope: "walk".into(),
    side,
    field: field.into(),
  };
  let mut tree = SourceDefinition::new("tree", fields);
  tree.traversal = Some(Box::new(TraversalDefinition {
    scope: "walk".into(),
    input,
    key: "id".into(),
    start: Expression::eq(
      Expression::field("nodes", "id"),
      Expression::parameter("start"),
    ),
    step: Expression::eq(
      row(TraversalSide::Current, "id"),
      row(TraversalSide::Next, "parentId"),
    ),
    cycles: CyclePolicy::Skip,
    path: Some(TraversalPathDefinition {
      node: "id".into(),
      prefix: None,
    }),
  }));
  let mut definition = GraphDefinition::new("tree", "tree");
  definition.sources = vec![tree];
  definition.parameters = serde_json::from_value::<Vec<ParameterDefinition>>(json!([
    { "name": "start", "scalarType": "int64", "required": true },
    { "name": "path", "scalarType": "int64", "shape": "list" }
  ]))
  .unwrap();
  definition.constraints = vec![ConstraintDefinition {
    when: ConstraintCondition::ParameterPresent {
      parameter: "path".into(),
    },
    predicate: Expression::TraversalPathEquals {
      source: "tree".into(),
      parameter: "path".into(),
    },
  }];
  definition.projection.fields = vec![
    ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("tree", "id"))
      .selected_by_default(),
    ProjectionFieldDefinition::new(
      vec!["rootId".into()],
      Expression::TraversalRoot {
        source: "tree".into(),
        field: "id".into(),
      },
    )
    .selected_by_default(),
    ProjectionFieldDefinition::new(
      vec!["level".into()],
      Expression::TraversalLevel {
        source: "tree".into(),
      },
    )
    .selected_by_default(),
  ];
  definition
}

fn mapping() -> RelationalMapping {
  serde_json::from_value(json!({ "sources": { "nodes": { "table": "Nodes" } } })).unwrap()
}

fn operation(size: usize) -> QueryOperation {
  serde_json::from_value(
    json!({ "parameters": { "start": 1, "path": (1..=size).collect::<Vec<_>>() } }),
  )
  .unwrap()
}

#[test]
fn traversal_source_compilation_is_thread_safe_and_operation_local() {
  let original = definition();
  let graph = original.clone().compile().unwrap();
  let mapping = mapping().compile(&graph).unwrap();
  std::thread::scope(|scope| {
    let handles: Vec<_> = (0..=8)
      .map(|size| {
        let graph = &graph;
        let mapping = &mapping;
        scope.spawn(move || {
          let statement = OracleCompiler::default()
            .compile(graph, mapping, &operation(size))
            .unwrap();
          assert_eq!(statement.bindings.len(), size + 1);
          assert_eq!(statement.bindings[0].parameter, "start");
          for (index, binding) in statement.bindings[1..].iter().enumerate() {
            assert_eq!(binding.parameter, "path");
            assert_eq!(binding.index, Some(index));
            assert_eq!(binding.path, None);
          }
          assert!(statement
            .sql
            .contains(&format!("\"t0\".\"qg_level\" = {size}")));
          assert!(statement.sql.contains("CONNECT_BY_ROOT"));
        })
      })
      .collect();
    for handle in handles {
      handle.join().unwrap();
    }
  });
  assert_eq!(graph.definition(), &original);
  let unfiltered: QueryOperation =
    serde_json::from_value(json!({ "parameters": { "start": 1 } })).unwrap();
  let statement = OracleCompiler::default()
    .compile(&graph, &mapping, &unfiltered)
    .unwrap();
  assert!(!statement.sql.contains("LEVEL <="));
  assert_eq!(statement.bindings.len(), 1);
}

#[test]
fn exact_paths_exceeding_one_case_expression_keep_all_elements() {
  let graph = definition().compile().unwrap();
  let mapping = mapping().compile(&graph).unwrap();
  let statement = OracleCompiler::default()
    .compile(&graph, &mapping, &operation(33_000))
    .unwrap();
  assert_eq!(statement.sql.matches("CASE LEVEL").count(), 2);
  assert!(statement.sql.contains("WHEN 33000 THEN :p33000"));
  assert!(statement.sql.contains("\"t0\".\"qg_level\" = 33000"));
  assert_eq!(statement.bindings.len(), 33_001);
  assert_eq!(statement.bindings.last().unwrap().index, Some(32_999));
}

#[test]
fn traversal_source_wire_round_trips_with_strict_nested_fields() {
  let original = definition();
  let mut wire = serde_json::to_value(&original).unwrap();
  let decoded: GraphDefinition = serde_json::from_value(wire.clone()).unwrap();
  assert_eq!(decoded, original);
  wire["sources"][0]["traversal"]["path"]["separator"] = json!("/");
  assert!(serde_json::from_value::<GraphDefinition>(wire).is_err());
  let graph = decoded.compile().unwrap();
  let mapping = mapping().compile(&graph).unwrap();
  assert!(matches!(
    SqlServerCompiler::default().compile(&graph, &mapping, &operation(3)),
    Err(SqlCompileError::UnsupportedDialectFeature {
      feature: "traversal source",
      ..
    })
  ));
}

#[test]
fn traversal_source_rejects_forged_metadata_types_and_scopes() {
  let mut malformed = definition();
  malformed.sources[0].fields[0].scalar_type = ScalarType::String;
  let issues = malformed.compile().unwrap_err();
  assert!(issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::InvalidTraversalDefinition));

  let mut malformed = definition();
  malformed.sources[0].traversal.as_mut().unwrap().step = Expression::eq(
    Expression::TraversalField {
      scope: "other".into(),
      side: TraversalSide::Current,
      field: "id".into(),
    },
    Expression::TraversalField {
      scope: "walk".into(),
      side: TraversalSide::Next,
      field: "parentId".into(),
    },
  );
  let issues = malformed.compile().unwrap_err();
  assert!(issues
    .as_slice()
    .iter()
    .any(|issue| issue.location == "sources[0].traversal.step"));

  let mut malformed = definition();
  malformed.parameters[1].scalar_type = ScalarType::String;
  assert!(malformed.compile().is_err());
}
