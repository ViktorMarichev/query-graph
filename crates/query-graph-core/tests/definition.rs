use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition,
  LiteralValue, OrderByDefinition, ParameterDefinition, ProjectionDefinition,
  ProjectionFieldDefinition, RelationDefinition, ScalarType, SourceDefinition,
};

fn attribute_value_definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("businessObjectRelationAttributeValues", "link");

  definition.sources = vec![
    SourceDefinition::new(
      "link",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("idOwner", ScalarType::Int64),
        FieldDefinition::new("idControllerObjectValue", ScalarType::Int64),
        FieldDefinition::new("idOrganisation", ScalarType::Int64),
        FieldDefinition::new("order", ScalarType::Int32),
      ],
    ),
    SourceDefinition::new(
      "value",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("idControllerObjectRequisite", ScalarType::Int64),
        FieldDefinition::new("value", ScalarType::String).nullable(),
        FieldDefinition::new("order", ScalarType::Int32),
      ],
    ),
    SourceDefinition::new(
      "requisite",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("code", ScalarType::String),
        FieldDefinition::new("name", ScalarType::String),
      ],
    ),
  ];

  definition.parameters = vec![
    ParameterDefinition::required("idOwner", ScalarType::Int64),
    ParameterDefinition::required("idOrganisation", ScalarType::Int64),
  ];

  definition.relations = vec![
    RelationDefinition::new(
      "value",
      "link",
      "value",
      Expression::eq(
        Expression::field("link", "idControllerObjectValue"),
        Expression::field("value", "id"),
      ),
    )
    .required(),
    RelationDefinition::new(
      "requisite",
      "value",
      "requisite",
      Expression::eq(
        Expression::field("value", "idControllerObjectRequisite"),
        Expression::field("requisite", "id"),
      ),
    )
    .required(),
  ];

  definition.constraints = vec![
    ConstraintDefinition::always(
      "owner",
      Expression::eq(
        Expression::field("link", "idOwner"),
        Expression::parameter("idOwner"),
      ),
    ),
    ConstraintDefinition::always(
      "organisation",
      Expression::eq(
        Expression::field("link", "idOrganisation"),
        Expression::parameter("idOrganisation"),
      ),
    ),
  ];

  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(
        vec!["value".into(), "id".into()],
        Expression::field("value", "id"),
      )
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["value".into(), "value".into()],
        Expression::field("value", "value"),
      )
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["value".into(), "requisite".into(), "name".into()],
        Expression::field("requisite", "name"),
      ),
    ],
  };

  definition.default_order_by = vec![
    OrderByDefinition::asc(Expression::field("link", "order")),
    OrderByDefinition::asc(Expression::field("value", "order")),
  ];

  definition
}

#[test]
fn compiles_and_indexes_a_semantic_graph_definition() {
  let graph = attribute_value_definition().compile().unwrap();

  assert_eq!(graph.root().key, "link");
  assert_eq!(
    graph.field("value", "value").unwrap().scalar_type,
    ScalarType::String
  );
  assert!(graph.parameter("idOwner").unwrap().required);
  assert_eq!(graph.relation("value").unwrap().to, "value");

  let outgoing: Vec<_> = graph
    .outgoing_relations("value")
    .unwrap()
    .map(|relation| relation.name.as_str())
    .collect();
  assert_eq!(outgoing, vec!["requisite"]);
}

#[test]
fn definition_ir_round_trips_through_json() {
  let definition = attribute_value_definition();
  let json = serde_json::to_string_pretty(&definition).unwrap();
  let restored: GraphDefinition = serde_json::from_str(&json).unwrap();

  assert_eq!(restored, definition);
  assert!(json.contains("\"schemaVersion\": 4"));
  assert!(json.contains("\"kind\": \"field\""));
  assert!(restored.compile().is_ok());
}

#[test]
fn reports_all_invalid_references_in_one_compilation() {
  let mut definition = attribute_value_definition();
  definition.relations[0].on = Expression::eq(
    Expression::field("missing", "id"),
    Expression::field("value", "missingField"),
  );
  definition.constraints[0].predicate = Expression::eq(
    Expression::field("link", "idOwner"),
    Expression::parameter("missingParameter"),
  );

  let issues = definition.compile().unwrap_err().into_vec();
  let codes: Vec<_> = issues.iter().map(|issue| issue.code).collect();

  assert!(codes.contains(&DefinitionIssueCode::UnknownFieldSource));
  assert!(codes.contains(&DefinitionIssueCode::UnknownField));
  assert!(codes.contains(&DefinitionIssueCode::UnknownParameter));
}

#[test]
fn rejects_projection_expression_across_relation_branches() {
  let mut definition = attribute_value_definition();
  definition.sources.push(SourceDefinition::new(
    "other",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  ));
  definition.relations.push(RelationDefinition::new(
    "other",
    "link",
    "other",
    Expression::eq(
      Expression::field("link", "id"),
      Expression::field("other", "id"),
    ),
  ));
  definition.projection.fields[0].expression = Expression::eq(
    Expression::field("value", "id"),
    Expression::field("other", "id"),
  );

  let issues = definition.compile().unwrap_err().into_vec();

  assert!(issues
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::ProjectionExpressionScope));
  assert!(issues
    .iter()
    .any(|issue| issue.message.contains("different relation branches")));
}

#[test]
fn rejects_sources_that_cannot_be_reached_from_the_root() {
  let mut definition = attribute_value_definition();
  definition.relations.pop();

  let issues = definition.compile().unwrap_err().into_vec();

  assert!(issues.iter().any(|issue| {
    issue.code == DefinitionIssueCode::UnreachableSource && issue.message.contains("requisite")
  }));
}

#[test]
fn rejects_invalid_decimal_literals_during_definition_compilation() {
  let mut definition = GraphDefinition::new("invalidDecimal", "root");
  definition.sources = vec![SourceDefinition::new(
    "root",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  )];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["amount".into()],
      Expression::literal(LiteralValue::Decimal("1e3".into())),
    )],
  };

  let issues = definition.compile().unwrap_err();

  assert!(issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::InvalidLiteral));
}
