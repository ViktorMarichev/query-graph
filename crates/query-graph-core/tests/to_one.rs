use std::collections::HashMap;

use query_graph_core::{
  DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition, LiteralValue,
  MappedQueryGraph, OperationIssueCode, OracleCompiler, OracleVersion, OrderByDefinition,
  ParameterDefinition, PlanError, ProjectionDefinition, ProjectionFieldDefinition, QueryOperation,
  RelationDefinition, RelationalMapping, ScalarType, SourceDefinition, SourceMapping,
  SqlCompileError,
};

fn definition(required: bool) -> GraphDefinition {
  let mut definition = GraphDefinition::new("staffCredentials", "staff");
  definition.sources = vec![
    SourceDefinition::new("staff", vec![FieldDefinition::new("id", ScalarType::Int64)]),
    SourceDefinition::new(
      "personStaff",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("idStaff", ScalarType::Int64),
        FieldDefinition::new("idPerson", ScalarType::Int64),
        FieldDefinition::new("isController", ScalarType::Int32),
      ],
    ),
  ];
  let relation = RelationDefinition::new(
    "credentials",
    "staff",
    "personStaff",
    Expression::and([
      Expression::eq(
        Expression::field("staff", "id"),
        Expression::field("personStaff", "idStaff"),
      ),
      Expression::eq(
        Expression::field("personStaff", "isController"),
        Expression::literal(LiteralValue::Integer(0)),
      ),
    ]),
  )
  .first_by([
    OrderByDefinition::asc(Expression::field("personStaff", "idPerson")),
    OrderByDefinition::asc(Expression::field("personStaff", "id")),
  ]);
  definition.relations = vec![if required {
    relation.required()
  } else {
    relation
  }];
  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("staff", "id"))
        .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["credentials".into(), "idPerson".into()],
        Expression::field("personStaff", "idPerson"),
      )
      .selected_by_default(),
    ],
  };
  definition.default_order_by = vec![OrderByDefinition::asc(Expression::field("staff", "id"))];
  definition
}

fn graph(required: bool) -> MappedQueryGraph {
  MappedQueryGraph::new(
    definition(required).compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("staff".into(), SourceMapping::new("Staff")),
        ("personStaff".into(), SourceMapping::new("PersonStaff")),
      ]),
    },
  )
  .unwrap()
}

#[test]
fn compiles_optional_first_by_to_sql_server_outer_apply() {
  let statement = graph(false)
    .compile_sql_server(&QueryOperation {
      limit: Some(20),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement.sql.contains(concat!(
    "OUTER APPLY (\n",
    "    SELECT TOP (1) [t1].*\n",
    "    FROM [PersonStaff] AS [t1]\n",
    "    WHERE (([t0].[id] = [t1].[idStaff]) AND ([t1].[isController] = 0))\n",
    "    ORDER BY [t1].[idPerson] ASC, [t1].[id] ASC\n",
    "  ) AS [t1]"
  )));
  assert!(statement
    .sql
    .ends_with("OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY"));
  assert!(statement.columns[1].nullable);
  assert_eq!(statement.relations[0].cardinality.as_str(), "one");
  assert!(!statement.relations[0].required);
}

#[test]
fn compiles_required_first_by_to_sql_server_cross_apply() {
  let statement = graph(true)
    .compile_sql_server(&QueryOperation::default())
    .unwrap();

  assert!(statement.sql.contains("\nCROSS APPLY ("));
  assert!(!statement.columns[1].nullable);
  assert!(statement.relations[0].required);
}

#[test]
fn compiles_first_by_to_oracle_12c_apply() {
  let statement = graph(false)
    .compile_oracle(&QueryOperation::default())
    .unwrap();

  assert!(statement.sql.contains(concat!(
    "OUTER APPLY (\n",
    "    SELECT \"t1\".*\n",
    "    FROM \"PersonStaff\" \"t1\"\n",
    "    WHERE ((\"t0\".\"id\" = \"t1\".\"idStaff\") AND (\"t1\".\"isController\" = 0))\n",
    "    ORDER BY \"t1\".\"idPerson\" ASC, \"t1\".\"id\" ASC\n",
    "    FETCH FIRST 1 ROW ONLY\n",
    "  ) \"t1\""
  )));
}

#[test]
fn reports_first_by_as_unsupported_on_oracle_11g() {
  let compiler = OracleCompiler::new(OracleVersion::V11g);
  let error = graph(false)
    .compile_oracle_with(&QueryOperation::default(), &compiler)
    .unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::UnsupportedDialectFeature {
      dialect: "Oracle",
      version: "11g",
      feature: "firstBy relation selection",
    }
  ));
}

#[test]
fn supports_first_by_relations_inside_exists_paths() {
  let mut definition = definition(false);
  definition.constraints = vec![query_graph_core::ConstraintDefinition::always(
    "hasCredentials",
    Expression::exists("personStaff"),
  )];
  definition.projection.fields.truncate(1);
  let graph = MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("staff".into(), SourceMapping::new("Staff")),
        ("personStaff".into(), SourceMapping::new("PersonStaff")),
      ]),
    },
  )
  .unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation::default())
    .unwrap();

  assert!(statement.sql.contains(concat!(
    "EXISTS (\n",
    "    SELECT 1\n",
    "    FROM (VALUES (1)) AS [__qg_seed]([value])\n",
    "    CROSS APPLY ("
  )));
  assert!(statement.relations.is_empty());
}

#[test]
fn validates_first_by_cardinality_order_and_scope() {
  let mut many = definition(false);
  many.relations[0].cardinality = query_graph_core::RelationCardinality::Many;
  let many_issues = many.compile().unwrap_err();
  assert!(many_issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::InvalidRelationSelection));

  let mut empty = definition(false);
  empty.relations[0].selection = Some(query_graph_core::RelationSelection::FirstBy {
    order_by: Vec::new(),
  });
  let empty_issues = empty.compile().unwrap_err();
  assert!(empty_issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::EmptyRelationSelectionOrder));

  let mut wrong_scope = definition(false);
  wrong_scope.relations[0].selection = Some(query_graph_core::RelationSelection::FirstBy {
    order_by: vec![OrderByDefinition::asc(Expression::field("staff", "id"))],
  });
  let scope_issues = wrong_scope.compile().unwrap_err();
  assert!(scope_issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::RelationSelectionExpressionScope));
}

#[test]
fn requires_parameters_used_by_first_by_inside_an_exists_path() {
  let mut definition = definition(false);
  definition.parameters = vec![ParameterDefinition::optional(
    "preferred",
    ScalarType::Int64,
  )];
  definition.relations[0].selection = Some(query_graph_core::RelationSelection::FirstBy {
    order_by: vec![
      OrderByDefinition::asc(Expression::parameter("preferred")),
      OrderByDefinition::asc(Expression::field("personStaff", "id")),
    ],
  });
  definition.constraints = vec![query_graph_core::ConstraintDefinition::always(
    "hasCredentials",
    Expression::exists("personStaff"),
  )];
  definition.projection.fields.truncate(1);
  let graph = MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("staff".into(), SourceMapping::new("Staff")),
        ("personStaff".into(), SourceMapping::new("PersonStaff")),
      ]),
    },
  )
  .unwrap();

  let error = graph
    .compile_sql_server(&QueryOperation::default())
    .unwrap_err();
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected operation validation error");
  };

  assert!(issues.as_slice().iter().any(|issue| {
    issue.code == OperationIssueCode::MissingParameter && issue.location == "parameters.preferred"
  }));
}
