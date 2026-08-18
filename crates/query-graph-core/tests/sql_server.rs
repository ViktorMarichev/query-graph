use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, LiteralValue,
  MappedQueryGraph, MappingIssueCode, OrderByDefinition, OrderingDefinition, ParameterDefinition,
  PlanError, ProjectionDefinition, ProjectionFieldDefinition, ProjectionObjectDefinition,
  QueryOperation, RelationCardinality, RelationDefinition, RelationalMapping, ScalarType,
  SemanticFunction, SourceDefinition, SourceMapping, SqlCompileError, SqlServerCompiler,
  SqlServerVersion, TableName,
};
use serde_json::json;

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("attributeValues", "link");
  definition.sources = vec![
    SourceDefinition::new(
      "link",
      vec![
        FieldDefinition::new("idOwner", ScalarType::Int64),
        FieldDefinition::new("idValue", ScalarType::Int64),
        FieldDefinition::new("order", ScalarType::Int32),
        FieldDefinition::new("dateDelete", ScalarType::DateTime).nullable(),
      ],
    ),
    SourceDefinition::new(
      "value",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("idDetail", ScalarType::Int64),
        FieldDefinition::new("value", ScalarType::String).nullable(),
        FieldDefinition::new("order", ScalarType::Int32),
      ],
    ),
    SourceDefinition::new(
      "detail",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("name", ScalarType::String),
      ],
    ),
  ];
  definition.parameters = vec![ParameterDefinition::required("idOwner", ScalarType::Int64)];
  definition.relations = vec![
    RelationDefinition::new(
      "value",
      "link",
      "value",
      Expression::eq(
        Expression::field("link", "idValue"),
        Expression::field("value", "id"),
      ),
    )
    .required(),
    RelationDefinition::new(
      "detail",
      "value",
      "detail",
      Expression::eq(
        Expression::field("value", "idDetail"),
        Expression::field("detail", "id"),
      ),
    ),
  ];
  definition.constraints = vec![
    ConstraintDefinition::always(Expression::eq(
      Expression::field("link", "idOwner"),
      Expression::parameter("idOwner"),
    )),
    ConstraintDefinition::always(Expression::IsNull {
      expression: Box::new(Expression::field("link", "dateDelete")),
    }),
  ];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
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
        vec!["value".into(), "detail".into(), "name".into()],
        Expression::field("detail", "name"),
      ),
    ],
  };
  definition.orderings = vec![OrderingDefinition::new(
    "default",
    [
      OrderByDefinition::asc(Expression::field("link", "order")),
      OrderByDefinition::asc(Expression::field("value", "order")),
    ],
  )
  .selected_by_default()];
  definition
}

fn mapping() -> RelationalMapping {
  RelationalMapping {
    sources: HashMap::from([
      (
        "link".into(),
        SourceMapping {
          table: TableName::Qualified {
            catalog: None,
            schema: Some("dbo".into()),
            name: "Attribute#Link".into(),
          },
          columns: HashMap::from([("idOwner".into(), "owner_id".into())]),
        },
      ),
      ("value".into(), SourceMapping::new("AttributeValue")),
      ("detail".into(), SourceMapping::new("AttributeDefinition")),
    ]),
  }
}

fn relational_graph() -> MappedQueryGraph {
  MappedQueryGraph::new(definition().compile().unwrap(), mapping()).unwrap()
}

#[test]
fn compiles_default_projection_to_sql_server() {
  let operation = QueryOperation {
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    offset: Some(10),
    limit: Some(25),
    ..QueryOperation::default()
  };

  let statement = relational_graph().compile_sql_server(&operation).unwrap();

  assert_eq!(
    statement.sql,
    concat!(
      "SELECT\n",
      "  [t1].[id] AS [c0],\n",
      "  [t1].[value] AS [c1]\n",
      "FROM [dbo].[Attribute#Link] AS [t0]\n",
      "INNER JOIN [AttributeValue] AS [t1]\n",
      "  ON ([t0].[idValue] = [t1].[id])\n",
      "WHERE\n",
      "  ([t0].[owner_id] = @p0)\n",
      "  AND ([t0].[dateDelete] IS NULL)\n",
      "ORDER BY\n",
      "  [t0].[order] ASC,\n",
      "  [t1].[order] ASC\n",
      "OFFSET 10 ROWS FETCH NEXT 25 ROWS ONLY"
    )
  );
  assert_eq!(statement.columns[0].name, "c0");
  assert_eq!(statement.columns[0].path, "value.id");
  assert_eq!(statement.columns[0].scalar_type, ScalarType::Int64);
  assert!(!statement.columns[0].nullable);
  assert_eq!(statement.columns[0].relations, ["value"]);
  assert_eq!(statement.columns[1].name, "c1");
  assert_eq!(statement.relations.len(), 1);
  assert_eq!(statement.relations[0].name, "value");
  assert_eq!(statement.relations[0].from, "link");
  assert_eq!(statement.relations[0].to, "value");
  assert_eq!(statement.relations[0].cardinality, RelationCardinality::One);
  assert!(statement.relations[0].required);
  assert_eq!(statement.bindings[0].name, "p0");
  assert_eq!(statement.bindings[0].parameter, "idOwner");
  assert_eq!(statement.bindings[0].scalar_type, ScalarType::Int64);
}

#[test]
fn enforces_sql_server_version_capabilities() {
  let graph = relational_graph();
  let compiler = SqlServerCompiler::new(SqlServerVersion::V2008);
  let operation = QueryOperation {
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    ..QueryOperation::default()
  };

  let statement = graph
    .compile_sql_server_with(&operation, &compiler)
    .unwrap();
  assert!(!statement.sql.contains("OFFSET"));

  let paginated = QueryOperation {
    offset: Some(1),
    limit: Some(10),
    ..operation
  };
  let error = graph
    .compile_sql_server_with(&paginated, &compiler)
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::UnsupportedDialectFeature {
      dialect: "SQL Server",
      version: "2008",
      feature: "OFFSET/FETCH pagination",
    }
  ));
}

#[test]
fn renders_concat_for_sql_server_2008_without_the_concat_function() {
  let mut definition = definition();
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![ProjectionFieldDefinition::new(
      vec!["value".into(), "label".into()],
      Expression::Function {
        name: SemanticFunction::Concat,
        arguments: vec![
          Expression::field("value", "value"),
          Expression::literal(LiteralValue::String(" suffix".into())),
        ],
      },
    )
    .selected_by_default()],
  };
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();
  let operation = QueryOperation {
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    ..QueryOperation::default()
  };

  let statement = graph
    .compile_sql_server_with(&operation, &SqlServerCompiler::new(SqlServerVersion::V2008))
    .unwrap();

  assert!(statement
    .sql
    .contains("(COALESCE([t1].[value], N'') + COALESCE(N' suffix', N'')) AS [c0]"));
  assert!(!statement.sql.contains("CONCAT("));
}

#[test]
fn plans_optional_join_for_an_explicit_projection() {
  let operation = QueryOperation {
    select: Some(vec!["value.detail.name".into()]),
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    ..QueryOperation::default()
  };

  let statement = relational_graph().compile_sql_server(&operation).unwrap();

  assert!(statement
    .sql
    .contains("LEFT JOIN [AttributeDefinition] AS [t2]"));
  assert!(statement.sql.contains("[t2].[name] AS [c0]"));
  assert_eq!(statement.columns[0].relations, ["value", "detail"]);
  assert_eq!(statement.columns[0].scalar_type, ScalarType::String);
  assert!(statement.columns[0].nullable);
}

#[test]
fn keeps_required_descendants_optional_below_an_optional_relation() {
  let mut definition = definition();
  definition.relations[0].required = false;
  definition.relations[1].required = true;
  definition.projection.fields[0] = ProjectionFieldDefinition::new(
    vec!["value".into(), "detail".into(), "name".into()],
    Expression::field("detail", "name"),
  )
  .selected_by_default();
  definition.projection.fields.truncate(1);
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement.sql.contains("LEFT JOIN [AttributeValue] AS [t1]"));
  assert!(statement
    .sql
    .contains("LEFT JOIN [AttributeDefinition] AS [t2]"));
  assert!(statement.columns[0].nullable);
  assert!(statement
    .relations
    .iter()
    .all(|relation| !relation.required));
}

#[test]
fn infers_the_deepest_path_for_a_multi_source_projection() {
  let mut definition = definition();
  definition.projection.fields = vec![ProjectionFieldDefinition::new(
    vec!["matches".into()],
    Expression::eq(
      Expression::field("link", "idOwner"),
      Expression::field("detail", "id"),
    ),
  )
  .selected_by_default()];
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert_eq!(statement.columns[0].relations, ["value", "detail"]);
  assert!(statement
    .sql
    .contains("LEFT JOIN [AttributeDefinition] AS [t2]"));
}

#[test]
fn rejects_an_unknown_selected_field() {
  let operation = QueryOperation {
    select: Some(vec!["value.missing".into()]),
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    ..QueryOperation::default()
  };

  let error = relational_graph()
    .compile_sql_server(&operation)
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::Plan(PlanError::Operation(_))
  ));
  assert!(error.to_string().contains("UnknownSelection"));
}

#[test]
fn requires_optional_parameters_referenced_by_the_active_plan() {
  let mut definition = definition();
  definition
    .parameters
    .push(ParameterDefinition::optional("label", ScalarType::String));
  definition.projection.fields.push(
    ProjectionFieldDefinition::new(vec!["label".into()], Expression::parameter("label"))
      .selected_by_default(),
  );
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let error = graph
    .compile_sql_server(&QueryOperation {
      select: Some(vec!["label".into()]),
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::Plan(PlanError::Operation(ref issues))
      if issues.as_slice().iter().any(|issue|
        issue.code == query_graph_core::OperationIssueCode::MissingParameter
          && issue.location == "parameters.label"
      )
  ));

  graph
    .compile_sql_server(&QueryOperation {
      select: Some(vec!["value.id".into()]),
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();
}

#[test]
fn requires_optional_parameters_referenced_by_an_active_constraint() {
  let mut definition = definition();
  definition.parameters.push(ParameterDefinition::optional(
    "filterOwner",
    ScalarType::Int64,
  ));
  definition
    .constraints
    .push(ConstraintDefinition::always(Expression::eq(
      Expression::field("link", "idOwner"),
      Expression::parameter("filterOwner"),
    )));
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let error = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap_err();

  assert!(error.to_string().contains("parameters.filterOwner"));
}

#[test]
fn reports_all_invalid_mapping_sources() {
  let mapping = RelationalMapping {
    sources: HashMap::from([("unknown".into(), SourceMapping::new("Unknown"))]),
  };

  let issues = mapping
    .compile(&definition().compile().unwrap())
    .unwrap_err()
    .into_vec();

  assert!(issues
    .iter()
    .any(|issue| issue.code == MappingIssueCode::UnknownSource));
  assert!(issues
    .iter()
    .any(|issue| issue.code == MappingIssueCode::MissingSource));
}

#[test]
fn plans_relation_paths_required_only_by_ordering() {
  let mut definition = definition();
  definition.projection.fields = vec![ProjectionFieldDefinition::new(
    vec!["owner".into()],
    Expression::field("link", "idOwner"),
  )
  .selected_by_default()];
  definition.orderings = vec![OrderingDefinition::new(
    "default",
    [OrderByDefinition::asc(Expression::field("detail", "name"))],
  )
  .selected_by_default()];
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement
    .sql
    .contains("INNER JOIN [AttributeValue] AS [t1]"));
  assert!(statement
    .sql
    .contains("LEFT JOIN [AttributeDefinition] AS [t2]"));
  assert!(statement.sql.contains("[t2].[name] ASC"));
}

#[test]
fn renders_sql_server_string_literals_as_unicode() {
  let mut definition = definition();
  definition.projection.fields[0].expression =
    Expression::literal(LiteralValue::String("O'Reilly".into()));
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement.sql.contains("N'O''Reilly' AS [c0]"));
}

#[test]
fn preserves_the_empty_schema_part_in_sql_server_table_names() {
  let mut mapping = mapping();
  mapping.sources.get_mut("link").unwrap().table = TableName::Qualified {
    catalog: Some("Application".into()),
    schema: None,
    name: "Attribute#Link".into(),
  };
  let graph = MappedQueryGraph::new(definition().compile().unwrap(), mapping).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement
    .sql
    .contains("FROM [Application]..[Attribute#Link] AS [t0]"));
}

#[test]
fn reports_many_cardinality_without_pagination() {
  let mut definition = definition();
  definition.relations[0].cardinality = RelationCardinality::Many;
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert_eq!(
    statement.relations[0].cardinality,
    RelationCardinality::Many
  );
}

#[test]
fn rejects_pagination_through_a_many_relation() {
  let mut definition = definition();
  definition.relations[0].cardinality = query_graph_core::RelationCardinality::Many;
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let error = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      limit: Some(25),
      ..QueryOperation::default()
    })
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::Plan(PlanError::PaginationThroughManyRelation { relation })
      if relation == "value"
  ));
}

#[test]
fn emits_presence_metadata_only_for_selected_projection_objects() {
  let mut definition = definition();
  definition.projection.objects = vec![
    ProjectionObjectDefinition::new(vec!["value".into()], Expression::field("value", "id")),
    ProjectionObjectDefinition::new(
      vec!["value".into(), "detail".into()],
      Expression::field("detail", "id"),
    ),
  ];
  definition
    .projection
    .fields
    .push(ProjectionFieldDefinition::new(
      vec!["owner".into()],
      Expression::field("link", "idOwner"),
    ));
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      select: Some(vec!["value.detail.name".into()]),
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert_eq!(statement.columns.len(), 1);
  assert_eq!(statement.columns[0].path, "value.detail.name");
  assert_eq!(statement.objects.len(), 2);
  assert_eq!(statement.objects[0].path, "value.detail");
  assert_eq!(statement.objects[0].presence_column, "o0");
  assert_eq!(statement.objects[1].path, "value");
  assert_eq!(statement.objects[1].presence_column, "o1");
  assert!(statement.sql.contains(concat!(
    "[t2].[name] AS [c0],\n",
    "  [t2].[id] AS [o0],\n",
    "  [t1].[id] AS [o1]"
  )));

  let value_only = graph
    .compile_sql_server(&QueryOperation {
      select: Some(vec!["owner".into()]),
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(value_only.objects.is_empty());
  assert!(!value_only.sql.contains("[AttributeDefinition] AS [t2]"));
}
