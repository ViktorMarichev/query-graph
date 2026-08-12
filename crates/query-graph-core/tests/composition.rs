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
