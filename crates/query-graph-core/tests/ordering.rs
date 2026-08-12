use std::collections::HashMap;

use query_graph_core::{
  DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition, MappedQueryGraph,
  OperationIssueCode, OrderByDefinition, OrderingDefinition, PlanError, ProjectionDefinition,
  ProjectionFieldDefinition, QueryOperation, RelationDefinition, RelationalMapping, ScalarType,
  SourceDefinition, SourceMapping, SqlCompileError,
};

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("staff", "staff");
  definition.sources = vec![
    SourceDefinition::new(
      "staff",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("name", ScalarType::String),
      ],
    ),
    SourceDefinition::new(
      "detail",
      vec![
        FieldDefinition::new("idStaff", ScalarType::Int64),
        FieldDefinition::new("label", ScalarType::String),
      ],
    ),
  ];
  definition.relations = vec![RelationDefinition::new(
    "detail",
    "staff",
    "detail",
    Expression::eq(
      Expression::field("staff", "id"),
      Expression::field("detail", "idStaff"),
    ),
  )];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![ProjectionFieldDefinition::new(
      vec!["id".into()],
      Expression::field("staff", "id"),
    )
    .selected_by_default()],
  };
  definition.orderings = vec![
    OrderingDefinition::new(
      "idAsc",
      [OrderByDefinition::asc(Expression::field("staff", "id"))],
    )
    .selected_by_default(),
    OrderingDefinition::new(
      "labelDesc",
      [OrderByDefinition::desc(Expression::field(
        "detail", "label",
      ))],
    ),
  ];
  definition
}

fn graph() -> MappedQueryGraph {
  MappedQueryGraph::new(
    definition().compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("staff".into(), SourceMapping::new("Staff")),
        ("detail".into(), SourceMapping::new("StaffDetail")),
      ]),
    },
  )
  .unwrap()
}

#[test]
fn selects_a_named_ordering_and_plans_only_its_relations() {
  let graph = graph();

  let default_statement = graph
    .compile_sql_server(&QueryOperation {
      limit: Some(10),
      ..QueryOperation::default()
    })
    .unwrap();
  assert!(default_statement.sql.contains("ORDER BY\n  [t0].[id] ASC"));
  assert!(!default_statement.sql.contains("StaffDetail"));
  assert!(default_statement.relations.is_empty());

  let operation = QueryOperation {
    ordering: Some("labelDesc".into()),
    ..QueryOperation::default()
  };
  let sql_server_statement = graph.compile_sql_server(&operation).unwrap();
  assert!(sql_server_statement
    .sql
    .contains("LEFT JOIN [StaffDetail] AS [t1]"));
  assert!(sql_server_statement
    .sql
    .contains("ORDER BY\n  [t1].[label] DESC"));
  assert_eq!(sql_server_statement.relations.len(), 1);

  let oracle_statement = graph.compile_oracle(&operation).unwrap();
  assert!(oracle_statement
    .sql
    .contains("LEFT JOIN \"StaffDetail\" \"t1\""));
  assert!(oracle_statement
    .sql
    .contains("ORDER BY\n  \"t1\".\"label\" DESC"));
}

#[test]
fn reports_an_unknown_operation_ordering() {
  let error = graph()
    .compile_sql_server(&QueryOperation {
      ordering: Some("missing".into()),
      ..QueryOperation::default()
    })
    .unwrap_err();

  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected operation issues");
  };
  assert!(issues.as_slice().iter().any(|issue| {
    issue.code == OperationIssueCode::UnknownOrdering && issue.location == "ordering"
  }));
}

#[test]
fn validates_ordering_names_content_and_default_selection() {
  let mut definition = definition();
  definition.orderings = vec![
    OrderingDefinition::new("", []),
    OrderingDefinition::new(
      "duplicate",
      [OrderByDefinition::asc(Expression::field("staff", "id"))],
    )
    .selected_by_default(),
    OrderingDefinition::new(
      "duplicate",
      [OrderByDefinition::desc(Expression::field("staff", "id"))],
    )
    .selected_by_default(),
  ];

  let issues = definition.compile().unwrap_err();
  let codes: Vec<_> = issues.as_slice().iter().map(|issue| issue.code).collect();

  assert!(codes.contains(&DefinitionIssueCode::EmptyOrderingName));
  assert!(codes.contains(&DefinitionIssueCode::EmptyOrdering));
  assert!(codes.contains(&DefinitionIssueCode::DuplicateOrdering));
  assert!(codes.contains(&DefinitionIssueCode::MultipleDefaultOrderings));
}
