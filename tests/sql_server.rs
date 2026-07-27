use std::collections::HashMap;

use query_graph::{
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, LiteralValue,
  MappedQueryGraph, MappingIssueCode, OrderByDefinition, ParameterDefinition, PlanError,
  ProjectionDefinition, ProjectionFieldDefinition, QueryOperation, RelationDefinition,
  RelationalMapping, ScalarType, SourceDefinition, SourceMapping, SqlCompileError, TableName,
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
    ConstraintDefinition::always(
      "owner",
      Expression::eq(
        Expression::field("link", "idOwner"),
        Expression::parameter("idOwner"),
      ),
    ),
    ConstraintDefinition::always(
      "active",
      Expression::IsNull {
        expression: Box::new(Expression::field("link", "dateDelete")),
      },
    ),
  ];
  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(
        vec!["value".into(), "id".into()],
        Expression::field("value", "id"),
      )
      .through(["value"])
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["value".into(), "value".into()],
        Expression::field("value", "value"),
      )
      .through(["value"])
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["value".into(), "detail".into(), "name".into()],
        Expression::field("detail", "name"),
      )
      .through(["value", "detail"]),
    ],
  };
  definition.default_order_by = vec![
    OrderByDefinition::asc(Expression::field("link", "order")),
    OrderByDefinition::asc(Expression::field("value", "order")),
  ];
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
            name: "Controller#Link".into(),
          },
          columns: HashMap::from([("idOwner".into(), "owner_id".into())]),
        },
      ),
      ("value".into(), SourceMapping::new("ControllerObjectValue")),
      ("detail".into(), SourceMapping::new("Requisite")),
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
      "  [value].[id] AS [value.id],\n",
      "  [value].[value] AS [value.value]\n",
      "FROM [dbo].[Controller#Link] AS [link]\n",
      "INNER JOIN [ControllerObjectValue] AS [value]\n",
      "  ON ([link].[idValue] = [value].[id])\n",
      "WHERE\n",
      "  ([link].[owner_id] = @p0)\n",
      "  AND ([link].[dateDelete] IS NULL)\n",
      "ORDER BY\n",
      "  [link].[order] ASC,\n",
      "  [value].[order] ASC\n",
      "OFFSET 10 ROWS FETCH NEXT 25 ROWS ONLY"
    )
  );
  assert_eq!(statement.fields, ["value.id", "value.value"]);
  assert_eq!(statement.bindings[0].name, "p0");
  assert_eq!(statement.bindings[0].parameter, "idOwner");
}

#[test]
fn plans_optional_join_for_an_explicit_projection() {
  let operation = QueryOperation {
    select: Some(vec!["value.detail.name".into()]),
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    ..QueryOperation::default()
  };

  let statement = relational_graph().compile_sql_server(&operation).unwrap();

  assert!(statement.sql.contains("LEFT JOIN [Requisite] AS [detail]"));
  assert!(statement
    .sql
    .contains("[detail].[name] AS [value.detail.name]"));
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
  definition.default_order_by = vec![OrderByDefinition::asc(Expression::field("detail", "name"))];
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement
    .sql
    .contains("INNER JOIN [ControllerObjectValue] AS [value]"));
  assert!(statement.sql.contains("LEFT JOIN [Requisite] AS [detail]"));
  assert!(statement.sql.contains("[detail].[name] ASC"));
}

#[test]
fn renders_sql_server_string_literals_as_unicode() {
  let mut definition = definition();
  definition.projection.fields[0].expression =
    Expression::literal(LiteralValue::String("O'Reilly".into()));
  definition.projection.fields[0].relations.clear();
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement.sql.contains("N'O''Reilly' AS [value.id]"));
}

#[test]
fn preserves_the_empty_schema_part_in_sql_server_table_names() {
  let mut mapping = mapping();
  mapping.sources.get_mut("link").unwrap().table = TableName::Qualified {
    catalog: Some("Controller".into()),
    schema: None,
    name: "Controller#Link".into(),
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
    .contains("FROM [Controller]..[Controller#Link] AS [link]"));
}
