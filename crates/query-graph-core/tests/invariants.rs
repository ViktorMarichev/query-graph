use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition,
  OrderByDefinition, ProjectionDefinition, ProjectionFieldDefinition, RelationDefinition,
  ScalarType, SemanticFunction, SourceDefinition, GRAPH_DEFINITION_VERSION,
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
  definition.schema_version = GRAPH_DEFINITION_VERSION - 1;
  definition.sources = vec![source("root")];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
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
    objects: Vec::new(),
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
    objects: Vec::new(),
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
    objects: Vec::new(),
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
fn rejects_exists_in_projections() {
  let mut definition = GraphDefinition::new("existsProjection", "root");
  definition.sources = vec![source("root"), source("child")];
  definition.relations = vec![relation("child", "root", "child")];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
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
fn requires_explicit_correlation_for_exists_in_relation_predicates() {
  let mut definition = GraphDefinition::new("relationExistsCorrelation", "root");
  definition.sources = vec![source("root"), source("child"), source("flag")];
  definition.relations = vec![
    RelationDefinition::new(
      "child",
      "root",
      "child",
      Expression::and([
        Expression::eq(
          Expression::field("root", "id"),
          Expression::field("child", "id"),
        ),
        Expression::exists("flag"),
      ]),
    ),
    relation("flag", "child", "flag"),
  ];

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::InvalidExistsSource));
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

#[test]
fn rejects_exists_correlation_from_a_deeper_source_without_panicking() {
  for target in ["middle", "root", "other"] {
    let mut definition = GraphDefinition::new("backwardExists", "root");
    definition.sources = vec![
      source("root"),
      source("middle"),
      source("leaf"),
      source("other"),
    ];
    definition.relations = vec![
      relation("middle", "root", "middle"),
      relation("leaf", "middle", "leaf"),
      relation("other", "root", "other"),
    ];
    definition.constraints = vec![ConstraintDefinition::always(Expression::exists_from(
      target, "leaf",
    ))];

    let issues = definition.compile().unwrap_err();
    let mut expected = vec![(
      DefinitionIssueCode::InvalidExistsSource,
      "constraints[0].predicate.from",
    )];
    if target == "root" {
      expected.push((
        DefinitionIssueCode::InvalidExistsSource,
        "constraints[0].predicate.source",
      ));
    }
    assert_eq!(
      issues
        .as_slice()
        .iter()
        .map(|issue| (issue.code, issue.location.as_str()))
        .collect::<Vec<_>>(),
      expected,
      "exists target {target:?} must return diagnostics"
    );
  }
}

#[test]
fn rejects_exists_paths_that_expand_the_containing_relation() {
  for target in ["child", "leaf"] {
    for selected in [false, true] {
      let mut definition = GraphDefinition::new("recursiveExists", "root");
      definition.sources = vec![source("root"), source("child"), source("leaf")];
      let mut child = relation("child", "root", "child");
      child.on = Expression::and([child.on, Expression::exists_from(target, "root")]);
      if selected {
        child = child.first_by([OrderByDefinition::asc(Expression::field("child", "id"))]);
      }
      definition.relations = vec![child, relation("leaf", "child", "leaf")];

      let issues = definition.compile().unwrap_err();
      assert_eq!(
        issues
          .as_slice()
          .iter()
          .map(|issue| (issue.code, issue.location.as_str()))
          .collect::<Vec<_>>(),
        vec![(DefinitionIssueCode::InvalidExistsContext, "relations[0].on")],
      );
    }
  }
}

#[test]
fn rejects_indirect_exists_dependency_cycles() {
  let mut definition = GraphDefinition::new("mutualExists", "root");
  definition.sources = vec![source("root"), source("left"), source("right")];
  definition.relations = vec![
    relation("left", "root", "left"),
    relation("right", "root", "right"),
  ];
  for (relation, target) in definition.relations.iter_mut().zip(["right", "left"]) {
    relation.on = Expression::and([relation.on.clone(), Expression::exists_from(target, "root")]);
  }

  let issues = definition.compile().unwrap_err();
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![(DefinitionIssueCode::InvalidExistsContext, "relations[1].on")],
  );
}

#[test]
fn checks_dependencies_inside_nested_exists_predicates() {
  let mut definition = GraphDefinition::new("nestedRecursiveExists", "root");
  definition.sources = vec![source("root"), source("left"), source("right")];
  definition.relations = vec![
    relation("left", "root", "left"),
    relation("right", "root", "right"),
  ];
  definition.relations[0].on = Expression::and([
    definition.relations[0].on.clone(),
    Expression::exists_from_where("right", "root", Expression::exists_from("left", "root")),
  ]);

  let issues = definition.compile().unwrap_err();
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![(DefinitionIssueCode::InvalidExistsContext, "relations[0].on")],
  );
}

#[test]
fn accepts_shared_acyclic_exists_dependencies() {
  let mut definition = GraphDefinition::new("sharedExists", "root");
  definition.sources = vec![
    source("root"),
    source("left"),
    source("right"),
    source("flag"),
  ];
  definition.relations = vec![
    relation("left", "root", "left"),
    relation("right", "root", "right"),
    relation("flag", "root", "flag"),
  ];
  for relation in &mut definition.relations[..2] {
    relation.on = Expression::and([
      relation.on.clone(),
      Expression::exists_from("flag", "root"),
      Expression::exists_from("flag", "root"),
    ]);
  }

  assert!(definition.compile().is_ok());
}

fn relation(name: &str, from: &str, to: &str) -> RelationDefinition {
  RelationDefinition::new(
    name,
    from,
    to,
    Expression::eq(Expression::field(from, "id"), Expression::field(to, "id")),
  )
}
