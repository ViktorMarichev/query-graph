use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, LiteralValue,
  MappedQueryGraph, NullsOrder, OracleCompiler, OracleVersion, OrderByDefinition,
  OrderingDefinition, ParameterDefinition, ProjectionDefinition, ProjectionFieldDefinition,
  QueryOperation, RelationDefinition, RelationalMapping, ScalarType, SemanticFunction,
  SourceDefinition, SourceMapping, SqlCompileError, TableName,
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
        FieldDefinition::new("order", ScalarType::Int32).nullable(),
        FieldDefinition::new("dateDelete", ScalarType::DateTime).nullable(),
      ],
    ),
    SourceDefinition::new(
      "value",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("value", ScalarType::String).nullable(),
      ],
    ),
  ];
  definition.parameters = vec![ParameterDefinition::required("idOwner", ScalarType::Int64)];
  definition.relations = vec![RelationDefinition::new(
    "value",
    "link",
    "value",
    Expression::eq(
      Expression::field("link", "idValue"),
      Expression::field("value", "id"),
    ),
  )
  .required()];
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
  definition.orderings = vec![OrderingDefinition::new(
    "default",
    [OrderByDefinition {
      expression: Expression::field("link", "order"),
      direction: query_graph_core::OrderDirection::Asc,
      nulls: Some(NullsOrder::Last),
    }],
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
            schema: Some("SOFTWARE".into()),
            name: "Controller#Link".into(),
          },
          columns: HashMap::from([("idOwner".into(), "owner_id".into())]),
        },
      ),
      (
        "value".into(),
        SourceMapping {
          table: TableName::Qualified {
            catalog: None,
            schema: Some("SOFTWARE".into()),
            name: "ControllerObjectValue".into(),
          },
          columns: HashMap::new(),
        },
      ),
    ]),
  }
}

fn relational_graph() -> MappedQueryGraph {
  MappedQueryGraph::new(definition().compile().unwrap(), mapping()).unwrap()
}

#[test]
fn compiles_the_common_query_plan_to_oracle() {
  let operation = QueryOperation {
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    offset: Some(10),
    limit: Some(25),
    ..QueryOperation::default()
  };

  let statement = relational_graph().compile_oracle(&operation).unwrap();

  assert_eq!(
    statement.sql,
    concat!(
      "SELECT\n",
      "  (\"t1\".\"value\" || ' suffix') AS \"c0\"\n",
      "FROM \"SOFTWARE\".\"Controller#Link\" \"t0\"\n",
      "INNER JOIN \"SOFTWARE\".\"ControllerObjectValue\" \"t1\"\n",
      "  ON (\"t0\".\"idValue\" = \"t1\".\"id\")\n",
      "WHERE\n",
      "  (\"t0\".\"owner_id\" = :p0)\n",
      "  AND (\"t0\".\"dateDelete\" IS NULL)\n",
      "ORDER BY\n",
      "  \"t0\".\"order\" ASC NULLS LAST\n",
      "OFFSET 10 ROWS FETCH NEXT 25 ROWS ONLY"
    )
  );
  assert_eq!(statement.columns[0].name, "c0");
  assert_eq!(statement.columns[0].path, "value.label");
  assert_eq!(statement.columns[0].scalar_type, ScalarType::String);
  assert!(!statement.columns[0].nullable);
  assert_eq!(statement.columns[0].relations, ["value"]);
  assert_eq!(statement.bindings[0].name, "p0");
  assert_eq!(statement.bindings[0].parameter, "idOwner");
}

#[test]
fn enforces_oracle_version_capabilities() {
  let graph = relational_graph();
  let compiler = OracleCompiler::new(OracleVersion::V11g);
  let operation = QueryOperation {
    parameters: HashMap::from([("idOwner".into(), json!(42))]),
    ..QueryOperation::default()
  };

  let statement = graph.compile_oracle_with(&operation, &compiler).unwrap();
  assert!(!statement.sql.contains("OFFSET"));

  let paginated = QueryOperation {
    offset: Some(1),
    limit: Some(10),
    ..operation
  };
  let error = graph
    .compile_oracle_with(&paginated, &compiler)
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::UnsupportedDialectFeature {
      dialect: "Oracle",
      version: "11g",
      feature: "OFFSET/FETCH pagination",
    }
  ));
}

#[test]
fn keeps_long_logical_names_out_of_oracle_identifiers() {
  const SOURCE: &str = "controller_attribute_value_link_logical_source";
  const PATH: &str = "controllerAttributeValueWithLongLogicalProjectionPath";

  let mut definition = GraphDefinition::new("physicalAliases", SOURCE);
  definition.sources = vec![SourceDefinition::new(
    SOURCE,
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  )];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![ProjectionFieldDefinition::new(
      vec![PATH.into()],
      Expression::field(SOURCE, "id"),
    )
    .selected_by_default()],
  };
  let graph = MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([(SOURCE.into(), SourceMapping::new("ShortTable"))]),
    },
  )
  .unwrap();

  let statement = graph.compile_oracle(&QueryOperation::default()).unwrap();

  assert_eq!(
    statement.sql,
    "SELECT\n  \"t0\".\"id\" AS \"c0\"\nFROM \"ShortTable\" \"t0\""
  );
  assert!(!statement.sql.contains(SOURCE));
  assert!(!statement.sql.contains(PATH));
  assert_eq!(statement.columns[0].path, PATH);
}

#[test]
fn rejects_a_sql_server_catalog_in_an_oracle_mapping() {
  let mut mapping = mapping();
  mapping.sources.get_mut("link").unwrap().table = TableName::Qualified {
    catalog: Some("Controller".into()),
    schema: None,
    name: "Controller#Link".into(),
  };
  let graph = MappedQueryGraph::new(definition().compile().unwrap(), mapping).unwrap();

  let error = graph
    .compile_oracle(&QueryOperation {
      parameters: HashMap::from([("idOwner".into(), json!(42))]),
      ..QueryOperation::default()
    })
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::UnsupportedTableQualifier {
      dialect: "Oracle",
      qualifier: "catalog"
    }
  ));
}
