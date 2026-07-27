use query_graph::{
  DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition, ProjectionDefinition,
  ProjectionFieldDefinition, RelationDefinition, ScalarType, SourceDefinition,
};

fn source(key: &str) -> SourceDefinition {
  SourceDefinition::new(key, vec![FieldDefinition::new("id", ScalarType::Int64)])
}

fn issue_codes(definition: GraphDefinition) -> Vec<DefinitionIssueCode> {
  definition
    .compile()
    .unwrap_err()
    .into_vec()
    .into_iter()
    .map(|issue| issue.code)
    .collect()
}

#[test]
fn rejects_projection_path_segments_containing_separator() {
  let mut definition = GraphDefinition::new("pathCollision", "root");
  definition.sources = vec![source("root")];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["nested.field".into()],
      Expression::field("root", "id"),
    )],
  };

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::InvalidProjectionPathSegment));
}

#[test]
fn rejects_selectable_projection_of_hidden_field() {
  let mut definition = GraphDefinition::new("hiddenField", "root");
  definition.sources = vec![SourceDefinition::new(
    "root",
    vec![FieldDefinition::new("secret", ScalarType::String).hidden()],
  )];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["secret".into()],
      Expression::Function {
        name: "lower".into(),
        arguments: vec![Expression::field("root", "secret")],
      },
    )],
  };

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::HiddenProjectionField));
}

#[test]
fn rejects_multiple_relation_paths_to_the_same_source() {
  let mut definition = GraphDefinition::new("ambiguousTopology", "root");
  definition.sources = vec![
    source("root"),
    source("left"),
    source("right"),
    source("target"),
  ];
  definition.relations = vec![
    relation("rootLeft", "root", "left"),
    relation("rootRight", "root", "right"),
    relation("leftTarget", "left", "target"),
    relation("rightTarget", "right", "target"),
  ];

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::AmbiguousSourcePath));
}

#[test]
fn rejects_relations_pointing_to_the_root() {
  let mut definition = GraphDefinition::new("rootCycle", "root");
  definition.sources = vec![source("root")];
  definition.relations = vec![relation("self", "root", "root")];

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::RootHasIncomingRelation));
}

fn relation(name: &str, from: &str, to: &str) -> RelationDefinition {
  RelationDefinition::new(
    name,
    from,
    to,
    Expression::eq(Expression::field(from, "id"), Expression::field(to, "id")),
  )
}
