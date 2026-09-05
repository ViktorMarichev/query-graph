use std::collections::HashMap;

use query_graph_core::{
  BatchRelationDefinition, ComposedCompileError, ComposedQueryGraph, CompositionIssueCode,
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, LiteralValue,
  MappedQueryGraph, OperationIssueCode, OrderByDefinition, OrderingDefinition, ParameterDefinition,
  ProjectionDefinition, ProjectionFieldDefinition, QueryOperation, RelationCardinality,
  RelationalMapping, ScalarType, SemanticFunction, SourceDefinition, SourceMapping,
  SqlServerCompiler, SqlServerVersion,
};
use serde_json::json;

fn root_graph() -> MappedQueryGraph {
  let mut definition = GraphDefinition::new("news", "news");
  definition.sources = vec![SourceDefinition::new(
    "news",
    vec![
      FieldDefinition::new("id", ScalarType::Int64),
      FieldDefinition::new("idAttachment", ScalarType::Int64),
      FieldDefinition::new("idOrganisation", ScalarType::Int64),
      FieldDefinition::new("profileName", ScalarType::String),
    ],
  )];
  definition.parameters = vec![ParameterDefinition::required(
    "idOrganisation",
    ScalarType::Int64,
  )];
  definition.constraints = vec![ConstraintDefinition::always(Expression::eq(
    Expression::field("news", "idOrganisation"),
    Expression::parameter("idOrganisation"),
  ))];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![
      ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("news", "id"))
        .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["idAttachment".into()],
        Expression::field("news", "idAttachment"),
      ),
      ProjectionFieldDefinition::new(
        vec!["profile".into(), "name".into()],
        Expression::field("news", "profileName"),
      ),
    ],
  };

  MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([("news".into(), SourceMapping::new("News"))]),
    },
  )
  .unwrap()
}

fn attachment_graph() -> MappedQueryGraph {
  attachment_graph_with(|_| {})
}

fn attachment_graph_with(customize: impl FnOnce(&mut GraphDefinition)) -> MappedQueryGraph {
  let mut definition = GraphDefinition::new("attachmentsByIds", "attachment");
  definition.sources = vec![SourceDefinition::new(
    "attachment",
    vec![
      FieldDefinition::new("idAttachment", ScalarType::Int64),
      FieldDefinition::new("path", ScalarType::String),
      FieldDefinition::new("kind", ScalarType::String),
    ],
  )];
  definition.parameters = vec![
    ParameterDefinition::required_list("ids", ScalarType::Int64),
    ParameterDefinition::required("kind", ScalarType::String),
  ];
  definition.constraints = vec![
    ConstraintDefinition::always(Expression::in_parameter(
      Expression::field("attachment", "idAttachment"),
      "ids",
    )),
    ConstraintDefinition::always(Expression::eq(
      Expression::field("attachment", "kind"),
      Expression::parameter("kind"),
    )),
  ];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![
      ProjectionFieldDefinition::new(
        vec!["idAttachment".into()],
        Expression::field("attachment", "idAttachment"),
      ),
      ProjectionFieldDefinition::new(vec!["path".into()], Expression::field("attachment", "path")),
      ProjectionFieldDefinition::new(
        vec!["display".into()],
        Expression::Function {
          name: SemanticFunction::Concat,
          arguments: vec![
            Expression::field("attachment", "path"),
            Expression::literal(LiteralValue::String(" preview".into())),
          ],
        },
      ),
    ],
  };
  definition.orderings = vec![OrderingDefinition::new(
    "pathAsc",
    [OrderByDefinition::asc(Expression::field(
      "attachment",
      "path",
    ))],
  )];
  customize(&mut definition);

  MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([("attachment".into(), SourceMapping::new("Attachment"))]),
    },
  )
  .unwrap()
}

fn relation(name: &str) -> BatchRelationDefinition {
  BatchRelationDefinition {
    name: name.into(),
    from: "idAttachment".into(),
    to: "idAttachment".into(),
    parameter: "ids".into(),
    cardinality: RelationCardinality::One,
    parameters: HashMap::from([("kind".into(), json!("preview"))]),
    ordering: Some("pathAsc".into()),
  }
}

#[test]
fn keeps_dotted_root_paths_and_defers_batch_compilation() {
  let child = attachment_graph();
  let graph = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(child.clone(), relation("preview"))
    .unwrap()
    .with_batch_relation(child, relation("badge"))
    .unwrap();
  let operation = QueryOperation {
    select: Some(vec![
      "profile.name".into(),
      "preview.display".into(),
      "badge.path".into(),
    ]),
    parameters: HashMap::from([("idOrganisation".into(), json!(7))]),
    ..QueryOperation::default()
  };

  let plan = graph
    .compile_sql_server_plan_with(&operation, &SqlServerCompiler::new(SqlServerVersion::V2008))
    .unwrap();

  assert_eq!(
    plan
      .root()
      .columns
      .iter()
      .map(|column| column.path.as_str())
      .collect::<Vec<_>>(),
    vec!["profile.name", "idAttachment"]
  );
  assert!(!plan.root().sql.contains("FROM [Attachment]"));

  let batches = plan.batches().collect::<Vec<_>>();
  assert_eq!(batches.len(), 2);
  assert!(batches.iter().all(|batch| batch.parent_key_injected));
  assert!(batches.iter().all(|batch| batch.child_key_injected));
  assert!(batches.iter().all(|batch| batch.key_parameter == "ids"));
  assert_eq!(batches[0].parameters.get("kind"), Some(&json!("preview")));

  let preview = plan
    .compile_batch("preview", &[json!(12), json!(18)])
    .unwrap();
  assert!(preview.sql.contains("FROM [Attachment]"));
  assert!(preview.sql.contains("COALESCE([t0].[path], N'') +"));
  assert_eq!(
    preview
      .columns
      .iter()
      .map(|column| column.path.as_str())
      .collect::<Vec<_>>(),
    vec!["display", "idAttachment"]
  );
  assert!(preview
    .bindings
    .iter()
    .any(|binding| binding.parameter == "ids" && binding.index == Some(1)));
  assert!(preview
    .bindings
    .iter()
    .any(|binding| binding.parameter == "kind"));
}

#[test]
fn reports_composition_errors_before_query_compilation() {
  let mut invalid = relation("profile");
  invalid.from = "missing".into();
  invalid.parameters.insert("kind".into(), json!(42));
  invalid.ordering = Some("missing".into());

  let issues = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(attachment_graph(), invalid)
    .unwrap_err();
  let codes = issues
    .as_slice()
    .iter()
    .map(|issue| issue.code)
    .collect::<Vec<_>>();

  assert!(codes.contains(&CompositionIssueCode::ConflictingProjectionPath));
  assert!(codes.contains(&CompositionIssueCode::UnknownParentKey));
  assert!(codes.contains(&CompositionIssueCode::InvalidStaticParameterType));
  assert!(codes.contains(&CompositionIssueCode::UnknownChildOrdering));
}

#[test]
fn preserves_ordering_and_static_parameter_diagnostics_in_validation_order() {
  let child = attachment_graph_with(|definition| {
    definition.parameters.extend([
      ParameterDefinition::required("locale", ScalarType::String),
      ParameterDefinition::optional_list("tags", ScalarType::String),
    ]);
  });
  let mut invalid = relation("preview");
  invalid.ordering = Some("missing".into());
  invalid.parameters = HashMap::from([
    ("kind".into(), json!(42)),
    ("tags".into(), json!(["valid", 42])),
    ("unknown".into(), json!(true)),
  ]);
  let issues = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(child.clone(), invalid.clone())
    .unwrap_err();
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![
      (
        CompositionIssueCode::UnknownChildOrdering,
        "relations[0].ordering"
      ),
      (
        CompositionIssueCode::UnknownStaticParameter,
        "relations[0].parameters.unknown"
      ),
      (
        CompositionIssueCode::InvalidStaticParameterType,
        "relations[0].parameters.kind"
      ),
      (
        CompositionIssueCode::MissingStaticParameter,
        "relations[0].parameters.locale"
      ),
      (
        CompositionIssueCode::InvalidStaticParameterType,
        "relations[0].parameters.tags[1]"
      ),
    ]
  );

  invalid.parameters.insert("ids".into(), json!([]));
  let error = child
    .compile_sql_server(&QueryOperation {
      select: Some(vec!["missing".into(), "path".into(), "path".into()]),
      ordering: invalid.ordering,
      parameters: invalid.parameters,
      limit: Some(0),
      ..QueryOperation::default()
    })
    .unwrap_err();
  let query_graph_core::SqlCompileError::Plan(query_graph_core::PlanError::Operation(issues)) =
    error
  else {
    panic!("expected operation validation error");
  };
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![
      (OperationIssueCode::UnknownSelection, "select[0]"),
      (OperationIssueCode::DuplicateSelection, "select[2]"),
      (OperationIssueCode::UnknownOrdering, "ordering"),
      (OperationIssueCode::UnknownParameter, "parameters.unknown"),
      (OperationIssueCode::InvalidParameterType, "parameters.kind"),
      (OperationIssueCode::MissingParameter, "parameters.locale"),
      (
        OperationIssueCode::InvalidParameterType,
        "parameters.tags[1]"
      ),
      (OperationIssueCode::InvalidPagination, "limit"),
    ]
  );
}

#[test]
fn preserves_key_diagnostics_without_duplicate_static_parameter_errors() {
  for (key_defined, expected_code) in [
    (false, CompositionIssueCode::UnknownKeyParameter),
    (true, CompositionIssueCode::KeyParameterNotList),
  ] {
    let child = attachment_graph_with(|definition| {
      definition.constraints.remove(0);
      if key_defined {
        definition.parameters[0] = ParameterDefinition::required("ids", ScalarType::Int64);
      } else {
        definition.parameters.remove(0);
      }
    });
    let issues = ComposedQueryGraph::new(root_graph())
      .with_batch_relation(child, relation("preview"))
      .unwrap_err();
    assert_eq!(
      issues
        .as_slice()
        .iter()
        .map(|issue| (issue.code, issue.location.as_str()))
        .collect::<Vec<_>>(),
      vec![(expected_code, "relations[0].parameter")]
    );
  }
}

#[test]
fn validates_static_configuration_even_when_the_child_key_projection_is_unknown() {
  let mut invalid = relation("preview");
  invalid.to = "missing".into();
  invalid.ordering = Some("missing".into());
  invalid.parameters.insert("kind".into(), json!(42));
  let issues = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(attachment_graph(), invalid)
    .unwrap_err();
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![
      (CompositionIssueCode::UnknownChildKey, "relations[0].to"),
      (
        CompositionIssueCode::UnknownChildOrdering,
        "relations[0].ordering"
      ),
      (
        CompositionIssueCode::InvalidStaticParameterType,
        "relations[0].parameters.kind"
      ),
    ]
  );
}

#[test]
fn preserves_the_static_validation_skip_for_a_child_without_projections() {
  let child = attachment_graph_with(|definition| definition.projection.fields.clear());
  let mut invalid = relation("preview");
  invalid.ordering = Some("missing".into());
  invalid.parameters.insert("kind".into(), json!(42));
  let issues = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(child, invalid)
    .unwrap_err();
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![(CompositionIssueCode::UnknownChildKey, "relations[0].to")]
  );
}

#[test]
fn preserves_the_key_location_prefix_filter_until_deferred_compilation() {
  let child = attachment_graph_with(|definition| {
    definition.parameters.push(ParameterDefinition::required(
      "ids[other]",
      ScalarType::String,
    ));
  });
  let mut definition = relation("preview");
  definition.parameters.insert("ids[other]".into(), json!(42));
  // This characterizes the existing prefix filter, not the intended parameter semantics.
  let graph = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(child, definition)
    .unwrap();
  let plan = graph
    .compile_sql_server_plan(&QueryOperation {
      select: Some(vec!["preview.path".into()]),
      parameters: HashMap::from([("idOrganisation".into(), json!(7))]),
      ..QueryOperation::default()
    })
    .unwrap();
  let error = plan.compile_batch("preview", &[json!(12)]).unwrap_err();
  let ComposedCompileError::Sql(query_graph_core::SqlCompileError::Plan(
    query_graph_core::PlanError::Operation(issues),
  )) = error
  else {
    panic!("expected deferred operation validation error");
  };
  assert_eq!(
    issues
      .as_slice()
      .iter()
      .map(|issue| (issue.code, issue.location.as_str()))
      .collect::<Vec<_>>(),
    vec![(
      OperationIssueCode::InvalidParameterType,
      "parameters.ids[other]"
    )]
  );
}

#[test]
fn validates_key_values_when_the_deferred_statement_is_compiled() {
  let graph = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(attachment_graph(), relation("preview"))
    .unwrap();
  let plan = graph
    .compile_sql_server_plan(&QueryOperation {
      select: Some(vec!["preview.path".into()]),
      parameters: HashMap::from([("idOrganisation".into(), json!(7))]),
      ..QueryOperation::default()
    })
    .unwrap();

  let error = plan
    .compile_batch("preview", &[json!("not-an-id")])
    .unwrap_err();
  let ComposedCompileError::Sql(query_graph_core::SqlCompileError::Plan(
    query_graph_core::PlanError::Operation(issues),
  )) = error
  else {
    panic!("expected operation validation error");
  };

  assert_eq!(
    issues.as_slice()[0].code,
    OperationIssueCode::InvalidParameterType
  );
  assert_eq!(issues.as_slice()[0].location, "parameters.ids[0]");
}

#[test]
fn compiles_only_explicitly_selected_batch_relations() {
  let graph = ComposedQueryGraph::new(root_graph())
    .with_batch_relation(attachment_graph(), relation("preview"))
    .unwrap();
  let plan = graph
    .compile_sql_server_plan(&QueryOperation {
      parameters: HashMap::from([("idOrganisation".into(), json!(7))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert_eq!(plan.batches().len(), 0);
  assert!(matches!(
    plan.compile_batch("preview", &[json!(12)]),
    Err(ComposedCompileError::UnknownSelectedBatchRelation(name)) if name == "preview"
  ));
}
