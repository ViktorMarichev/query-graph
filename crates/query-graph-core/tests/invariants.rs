use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition,
  ProjectionDefinition, ProjectionFieldDefinition, RelationDefinition, ScalarType,
  SemanticFunction, SourceDefinition,
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
  definition.schema_version = 7;
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

#[test]
fn rejects_exists_outside_graph_constraints() {
  let mut definition = GraphDefinition::new("existsProjection", "root");
  definition.sources = vec![source("root"), source("child")];
  definition.relations = vec![relation("child", "root", "child")];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["hasChild".into()],
      Expression::exists("child"),
    )],
  };

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::InvalidExistsContext));
}

#[test]
fn validates_exists_target_and_predicate_scope() {
  let mut definition = GraphDefinition::new("existsScope", "root");
  definition.sources = vec![source("root"), source("left"), source("right")];
  definition.relations = vec![
    relation("left", "root", "left"),
    relation("right", "root", "right"),
  ];
  definition.constraints = vec![
    ConstraintDefinition::always(Expression::exists("root")),
    ConstraintDefinition::always(Expression::exists("missing")),
    ConstraintDefinition::always(Expression::exists_where(
      "left",
      Expression::eq(
        Expression::field("left", "id"),
        Expression::field("right", "id"),
      ),
    )),
  ];

  let codes = issue_codes(definition);
  assert!(codes.contains(&DefinitionIssueCode::InvalidExistsSource));
  assert!(codes.contains(&DefinitionIssueCode::UnknownExistsSource));
  assert!(codes.contains(&DefinitionIssueCode::ExistsExpressionScope));
}

#[test]
fn rejects_a_non_boolean_exists_predicate() {
  let mut definition = GraphDefinition::new("existsPredicateType", "root");
  definition.sources = vec![source("root"), source("child")];
  definition.relations = vec![relation("child", "root", "child")];
  definition.constraints = vec![ConstraintDefinition::always(Expression::exists_where(
    "child",
    Expression::field("child", "id"),
  ))];

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::InvalidPredicateType));
}

#[test]
fn accepts_exists_predicates_over_the_inferred_root_path() {
  let mut definition = GraphDefinition::new("existsPath", "root");
  definition.sources = vec![source("root"), source("middle"), source("target")];
  definition.relations = vec![
    relation("middle", "root", "middle"),
    relation("target", "middle", "target"),
  ];
  definition.constraints = vec![ConstraintDefinition::always(Expression::exists_where(
    "target",
    Expression::eq(
      Expression::field("middle", "id"),
      Expression::field("target", "id"),
    ),
  ))];

  assert!(definition.compile().is_ok());
}

#[test]
fn validates_exists_correlation_source() {
  let mut definition = GraphDefinition::new("existsFrom", "root");
  definition.sources = vec![source("root"), source("left"), source("right")];
  definition.relations = vec![
    relation("left", "root", "left"),
    relation("right", "root", "right"),
  ];
  definition.constraints = vec![
    ConstraintDefinition::always(Expression::exists_from("right", "left")),
    ConstraintDefinition::always(Expression::exists_from("right", "missing")),
  ];

  let codes = issue_codes(definition);
  assert!(codes.contains(&DefinitionIssueCode::InvalidExistsSource));
  assert!(codes.contains(&DefinitionIssueCode::UnknownExistsSource));
}

fn relation(name: &str, from: &str, to: &str) -> RelationDefinition {
  RelationDefinition::new(
    name,
    from,
    to,
    Expression::eq(Expression::field(from, "id"), Expression::field(to, "id")),
  )
}
