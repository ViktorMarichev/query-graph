use std::collections::HashMap;

use query_graph_core::{
  AggregateFunction, ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition,
  GraphDefinition, MappedQueryGraph, OrderByDefinition, ParameterDefinition, PlanError,
  ProjectionDefinition, ProjectionFieldDefinition, QueryOperation, RelationDefinition,
  RelationalMapping, ScalarType, SourceDefinition, SourceMapping, SqlCompileError,
};
use serde_json::json;

fn staff_count() -> Expression {
  Expression::count_distinct(Expression::field("staff", "idStaff"))
}

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("serviceSummary", "service");
  definition.sources = vec![
    SourceDefinition::new(
      "service",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("idOrganisation", ScalarType::Int64),
      ],
    ),
    SourceDefinition::new(
      "staff",
      vec![
        FieldDefinition::new("idService", ScalarType::Int64),
        FieldDefinition::new("idStaff", ScalarType::Int64),
        FieldDefinition::new("hours", ScalarType::Decimal).nullable(),
      ],
    ),
  ];
  definition.parameters = vec![
    ParameterDefinition::required("idOrganisation", ScalarType::Int64),
    ParameterDefinition::required("minimumStaff", ScalarType::Int64),
  ];
  definition.relations = vec![RelationDefinition::new(
    "staff",
    "service",
    "staff",
    Expression::eq(
      Expression::field("service", "id"),
      Expression::field("staff", "idService"),
    ),
  )
  .many()];
  definition.constraints = vec![
    ConstraintDefinition::always(
      "organisation",
      Expression::eq(
        Expression::field("service", "idOrganisation"),
        Expression::parameter("idOrganisation"),
      ),
    ),
    ConstraintDefinition::always(
      "minimumStaff",
      Expression::GreaterThanOrEqual {
        left: Box::new(staff_count()),
        right: Box::new(Expression::parameter("minimumStaff")),
      },
    ),
  ];
  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(vec!["serviceId".into()], Expression::field("service", "id"))
        .dimension()
        .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["staffRows".into()],
        Expression::count_of(Expression::field("staff", "idStaff")),
      )
      .measure()
      .selected_by_default(),
      ProjectionFieldDefinition::new(vec!["staffCount".into()], staff_count())
        .measure()
        .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["totalHours".into()],
        Expression::sum(Expression::field("staff", "hours")),
      )
      .measure()
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["averageHours".into()],
        Expression::average(Expression::field("staff", "hours")),
      )
      .measure()
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["minimumHours".into()],
        Expression::minimum(Expression::field("staff", "hours")),
      )
      .measure()
      .selected_by_default(),
      ProjectionFieldDefinition::new(
        vec!["maximumHours".into()],
        Expression::maximum(Expression::field("staff", "hours")),
      )
      .measure()
      .selected_by_default(),
    ],
  };
  definition.default_order_by = vec![OrderByDefinition::desc(staff_count())];
  definition
}

fn relational_graph() -> MappedQueryGraph {
  MappedQueryGraph::new(
    definition().compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("service".into(), SourceMapping::new("Service")),
        ("staff".into(), SourceMapping::new("ServiceStaff")),
      ]),
    },
  )
  .unwrap()
}

fn operation() -> QueryOperation {
  QueryOperation {
    parameters: HashMap::from([
      ("idOrganisation".into(), json!(7)),
      ("minimumStaff".into(), json!(2)),
    ]),
    offset: Some(0),
    limit: Some(25),
    ..QueryOperation::default()
  }
}

#[test]
fn compiles_summary_semantics_to_sql_server() {
  let statement = relational_graph().compile_sql_server(&operation()).unwrap();

  assert_eq!(
    statement.sql,
    concat!(
      "SELECT\n",
      "  [t0].[id] AS [c0],\n",
      "  COUNT_BIG([t1].[idStaff]) AS [c1],\n",
      "  COUNT_BIG(DISTINCT [t1].[idStaff]) AS [c2],\n",
      "  SUM([t1].[hours]) AS [c3],\n",
      "  AVG((1.0 * [t1].[hours])) AS [c4],\n",
      "  MIN([t1].[hours]) AS [c5],\n",
      "  MAX([t1].[hours]) AS [c6]\n",
      "FROM [Service] AS [t0]\n",
      "LEFT JOIN [ServiceStaff] AS [t1]\n",
      "  ON ([t0].[id] = [t1].[idService])\n",
      "WHERE\n",
      "  ([t0].[idOrganisation] = @p0)\n",
      "GROUP BY\n",
      "  [t0].[id]\n",
      "HAVING\n",
      "  (COUNT_BIG(DISTINCT [t1].[idStaff]) >= @p1)\n",
      "ORDER BY\n",
      "  COUNT_BIG(DISTINCT [t1].[idStaff]) DESC\n",
      "OFFSET 0 ROWS FETCH NEXT 25 ROWS ONLY"
    )
  );
  assert_eq!(statement.bindings[0].parameter, "idOrganisation");
  assert_eq!(statement.bindings[1].parameter, "minimumStaff");
  assert_eq!(statement.columns[1].scalar_type, ScalarType::Int64);
  assert!(!statement.columns[1].nullable);
  assert_eq!(statement.columns[1].relations, ["staff"]);
  assert_eq!(statement.columns[3].scalar_type, ScalarType::Decimal);
  assert!(statement.columns[3].nullable);
  assert_eq!(statement.columns[4].scalar_type, ScalarType::Decimal);
  assert!(statement.columns[4].nullable);
}

#[test]
fn compiles_the_same_summary_semantics_to_oracle() {
  let statement = relational_graph().compile_oracle(&operation()).unwrap();

  assert!(statement
    .sql
    .contains("COUNT(DISTINCT \"t1\".\"idStaff\") AS \"c2\""));
  assert!(statement
    .sql
    .contains("AVG((1.0 * \"t1\".\"hours\")) AS \"c4\""));
  assert!(statement.sql.contains(
    "WHERE\n  (\"t0\".\"idOrganisation\" = :p0)\nGROUP BY\n  \"t0\".\"id\"\nHAVING\n  (COUNT(DISTINCT \"t1\".\"idStaff\") >= :p1)"
  ));
  assert!(statement
    .sql
    .contains("ORDER BY\n  COUNT(DISTINCT \"t1\".\"idStaff\") DESC"));
}

#[test]
fn renders_count_without_an_expression_as_count_all() {
  let mut definition = definition();
  definition.projection.fields =
    vec![
      ProjectionFieldDefinition::new(vec!["rows".into()], Expression::count())
        .measure()
        .selected_by_default(),
    ];
  definition.default_order_by.clear();
  let graph = MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("service".into(), SourceMapping::new("Service")),
        ("staff".into(), SourceMapping::new("ServiceStaff")),
      ]),
    },
  )
  .unwrap();

  let statement = graph
    .compile_sql_server(&QueryOperation {
      parameters: operation().parameters,
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement.sql.starts_with("SELECT\n  COUNT_BIG(*) AS [c0]"));
}

#[test]
fn rejects_aggregate_expressions_in_a_record_graph() {
  let mut definition = GraphDefinition::new("invalidRecord", "root");
  definition.sources = vec![SourceDefinition::new(
    "root",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  )];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["count".into()],
      Expression::count(),
    )],
  };

  let issues = definition.compile().unwrap_err();

  assert!(issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::InvalidAggregateContext));
}

#[test]
fn rejects_invalid_summary_roles_and_group_scope() {
  let mut definition = definition();
  definition.projection.fields[0].expression = staff_count();
  definition.projection.fields[1].expression = Expression::field("staff", "idStaff");
  definition
    .projection
    .fields
    .push(ProjectionFieldDefinition::new(
      vec!["regularOrganisation".into()],
      Expression::field("service", "idOrganisation"),
    ));
  definition.default_order_by = vec![OrderByDefinition::asc(Expression::field(
    "service",
    "idOrganisation",
  ))];

  let issues = definition.compile().unwrap_err();
  let codes: Vec<_> = issues.as_slice().iter().map(|issue| issue.code).collect();

  assert!(codes.contains(&DefinitionIssueCode::MixedProjectionRoles));
  assert!(codes.contains(&DefinitionIssueCode::InvalidDimensionExpression));
  assert!(codes.contains(&DefinitionIssueCode::InvalidMeasureExpression));
  assert!(codes.contains(&DefinitionIssueCode::UngroupedExpression));
}

#[test]
fn rejects_nested_aggregates() {
  let mut definition = definition();
  definition.projection.fields = vec![ProjectionFieldDefinition::new(
    vec!["nested".into()],
    Expression::sum(Expression::count_of(Expression::field("staff", "idStaff"))),
  )
  .measure()];
  definition.default_order_by.clear();

  let issues = definition.compile().unwrap_err();

  assert!(issues
    .as_slice()
    .iter()
    .any(|issue| issue.code == DefinitionIssueCode::NestedAggregate));
}

#[test]
fn validates_aggregate_arguments_and_result_types() {
  let graph = definition().compile().unwrap();

  let staff_count = graph.projection_type("staffCount").unwrap();
  assert_eq!(staff_count.scalar_type, ScalarType::Int64);
  assert!(!staff_count.nullable);

  let average_hours = graph.projection_type("averageHours").unwrap();
  assert_eq!(average_hours.scalar_type, ScalarType::Decimal);
  assert!(average_hours.nullable);

  let mut invalid = definition();
  invalid.sources[0]
    .fields
    .push(FieldDefinition::new("label", ScalarType::String));
  invalid.sources[0]
    .fields
    .push(FieldDefinition::new("enabled", ScalarType::Boolean));
  invalid.projection.fields = vec![
    ProjectionFieldDefinition::new(
      vec!["missingArgument".into()],
      Expression::aggregate(AggregateFunction::Sum, None),
    )
    .measure(),
    ProjectionFieldDefinition::new(
      vec!["invalidSum".into()],
      Expression::sum(Expression::field("service", "label")),
    )
    .measure(),
    ProjectionFieldDefinition::new(
      vec!["invalidMinimum".into()],
      Expression::minimum(Expression::field("service", "enabled")),
    )
    .measure(),
  ];
  invalid.default_order_by.clear();

  let issues = invalid.compile().unwrap_err();
  let codes: Vec<_> = issues.as_slice().iter().map(|issue| issue.code).collect();

  assert!(codes.contains(&DefinitionIssueCode::InvalidFunctionArity));
  assert!(codes.contains(&DefinitionIssueCode::InvalidExpressionType));
}
#[test]
fn rejects_predicate_aggregate_arguments() {
  let mut definition = definition();
  definition.projection.fields = vec![ProjectionFieldDefinition::new(
    vec!["predicateCount".into()],
    Expression::count_of(Expression::eq(
      Expression::field("service", "id"),
      Expression::field("staff", "idService"),
    )),
  )
  .measure()];
  definition.default_order_by.clear();

  let issues = definition.compile().unwrap_err();

  assert!(issues.as_slice().iter().any(|issue| {
    issue.code == DefinitionIssueCode::InvalidAggregateContext
      && issue
        .message
        .contains("predicate expressions cannot be aggregate arguments")
  }));
}

#[test]
fn rejects_dimensions_without_equality_semantics() {
  let mut definition = definition();
  definition.sources[0]
    .fields
    .push(FieldDefinition::new("metadata", ScalarType::Json));
  definition.projection.fields = vec![
    ProjectionFieldDefinition::new(
      vec!["metadata".into()],
      Expression::field("service", "metadata"),
    )
    .dimension(),
    ProjectionFieldDefinition::new(vec!["rows".into()], Expression::count()).measure(),
  ];
  definition.default_order_by.clear();

  let issues = definition.compile().unwrap_err();

  assert!(issues.as_slice().iter().any(|issue| {
    issue.code == DefinitionIssueCode::InvalidDimensionExpression
      && issue.message.contains("must have equality semantics")
  }));
}

#[test]
fn rejects_aggregation_across_independent_many_branches() {
  let mut definition = definition();
  definition.sources.push(SourceDefinition::new(
    "tag",
    vec![
      FieldDefinition::new("idService", ScalarType::Int64),
      FieldDefinition::new("idTag", ScalarType::Int64),
    ],
  ));
  definition.relations.push(
    RelationDefinition::new(
      "tags",
      "service",
      "tag",
      Expression::eq(
        Expression::field("service", "id"),
        Expression::field("tag", "idService"),
      ),
    )
    .many(),
  );
  definition.projection.fields.push(
    ProjectionFieldDefinition::new(
      vec!["tagCount".into()],
      Expression::count_distinct(Expression::field("tag", "idTag")),
    )
    .measure()
    .selected_by_default(),
  );

  let graph = MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([
        ("service".into(), SourceMapping::new("Service")),
        ("staff".into(), SourceMapping::new("ServiceStaff")),
        ("tag".into(), SourceMapping::new("ServiceTag")),
      ]),
    },
  )
  .unwrap();

  let error = graph.compile_sql_server(&operation()).unwrap_err();

  assert!(matches!(
    error,
    SqlCompileError::Plan(PlanError::AggregationAcrossManyBranches {
      ref left_relation,
      ref right_relation,
    }) if left_relation == "staff" && right_relation == "tags"
  ));
}
