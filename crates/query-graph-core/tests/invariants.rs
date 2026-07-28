use query_graph_core::{
  DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition, ProjectionDefinition,
  ProjectionFieldDefinition, RelationDefinition, ScalarType, SemanticFunction, SourceDefinition,
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
fn rejects_the_previous_wire_definition_version() {
  let mut definition = GraphDefinition::new("oldWireVersion", "root");
  definition.schema_version = 2;
  definition.sources = vec![source("root")];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["id".into()],
      Expression::field("root", "id"),
    )],
  };

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::UnsupportedVersion));
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
fn rejects_overlapping_projection_paths() {
  let mut definition = GraphDefinition::new("pathConflict", "root");
  definition.sources = vec![source("root")];
  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(vec!["owner".into()], Expression::field("root", "id")),
      ProjectionFieldDefinition::new(
        vec!["owner".into(), "id".into()],
        Expression::field("root", "id"),
      ),
    ],
  };

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::ConflictingProjectionPath));
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
        name: SemanticFunction::Lower,
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
