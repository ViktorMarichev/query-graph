use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, MappedQueryGraph,
  OperationIssueCode, OrderByDefinition, OrderingDefinition, ParameterDefinition, PlanError,
  ProjectionDefinition, ProjectionFieldDefinition, QueryOperation, RelationDefinition,
  RelationalMapping, ScalarType, SourceDefinition, SourceMapping, SqlCompileError,
};
use serde_json::json;

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("membersWithEventAccess", "staff");
  definition.sources = vec![
    SourceDefinition::new("staff", vec![FieldDefinition::new("id", ScalarType::Int64)]),
    SourceDefinition::new(
      "staffRole",
      vec![
        FieldDefinition::new("idStaff", ScalarType::Int64),
        FieldDefinition::new("idRole", ScalarType::Int64),
      ],
    ),
    SourceDefinition::new(
      "accessRole",
      vec![FieldDefinition::new("id", ScalarType::Int64)],
    ),
    SourceDefinition::new(
      "accessEventRole",
      vec![
        FieldDefinition::new("idAccessRole", ScalarType::Int64),
        FieldDefinition::new("idEvent", ScalarType::Int64),
      ],
    ),
    SourceDefinition::new(
      "event",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("code", ScalarType::String),
      ],
    ),
  ];
  definition.parameters = vec![ParameterDefinition::required(
    "eventCode",
    ScalarType::String,
  )];
  definition.relations = vec![
    RelationDefinition::new(
      "staffRoles",
      "staff",
      "staffRole",
      Expression::eq(
        Expression::field("staff", "id"),
        Expression::field("staffRole", "idStaff"),
      ),
    )
    .many(),
    RelationDefinition::new(
      "accessRole",
      "staffRole",
      "accessRole",
      Expression::eq(
        Expression::field("staffRole", "idRole"),
        Expression::field("accessRole", "id"),
      ),
    ),
    RelationDefinition::new(
      "accessEventRoles",
      "accessRole",
      "accessEventRole",
      Expression::eq(
        Expression::field("accessRole", "id"),
        Expression::field("accessEventRole", "idAccessRole"),
      ),
    )
    .many(),
    RelationDefinition::new(
      "event",
      "accessEventRole",
      "event",
      Expression::eq(
        Expression::field("accessEventRole", "idEvent"),
        Expression::field("event", "id"),
      ),
    ),
  ];
  definition.constraints = vec![ConstraintDefinition::always(Expression::exists_where(
    "event",
    Expression::eq(
      Expression::field("event", "code"),
      Expression::parameter("eventCode"),
    ),
  ))];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![ProjectionFieldDefinition::new(
      vec!["id".into()],
      Expression::field("staff", "id"),
    )
    .selected_by_default()],
  };
  definition.orderings = vec![OrderingDefinition::new(
    "default",
    [OrderByDefinition::asc(Expression::field("staff", "id"))],
  )
  .selected_by_default()];
  definition
}

fn mapping() -> RelationalMapping {
  RelationalMapping {
    sources: HashMap::from([
      ("staff".into(), SourceMapping::new("Staff")),
      ("staffRole".into(), SourceMapping::new("StaffRole")),
      ("accessRole".into(), SourceMapping::new("AccessRole")),
      (
        "accessEventRole".into(),
        SourceMapping::new("AccessEventRole"),
      ),
      ("event".into(), SourceMapping::new("Event")),
    ]),
  }
}

fn operation() -> QueryOperation {
  QueryOperation {
    parameters: HashMap::from([("eventCode".into(), json!("PROVISION_OF_SERVICE_RB"))]),
    limit: Some(25),
    ..QueryOperation::default()
  }
}

fn graph() -> MappedQueryGraph {
  MappedQueryGraph::new(definition().compile().unwrap(), mapping()).unwrap()
}

#[test]
fn compiles_exists_to_sql_server_without_materializing_many_relations() {
  let statement = graph().compile_sql_server(&operation()).unwrap();

  assert_eq!(
    statement.sql,
    concat!(
      "SELECT\n",
      "  [t0].[id] AS [c0]\n",
      "FROM [Staff] AS [t0]\n",
      "WHERE\n",
      "  EXISTS (\n",
      "    SELECT 1\n",
      "    FROM [StaffRole] AS [t1]\n",
      "    INNER JOIN [AccessRole] AS [t2]\n",
      "      ON ([t1].[idRole] = [t2].[id])\n",
      "    INNER JOIN [AccessEventRole] AS [t3]\n",
      "      ON ([t2].[id] = [t3].[idAccessRole])\n",
      "    INNER JOIN [Event] AS [t4]\n",
      "      ON ([t3].[idEvent] = [t4].[id])\n",
      "    WHERE ([t0].[id] = [t1].[idStaff])\n",
      "      AND ([t4].[code] = @p0)\n",
      "  )\n",
      "ORDER BY\n",
      "  [t0].[id] ASC\n",
      "OFFSET 0 ROWS FETCH NEXT 25 ROWS ONLY"
    )
  );
  assert!(statement.relations.is_empty());
  assert!(statement.columns[0].relations.is_empty());
  assert_eq!(statement.bindings[0].parameter, "eventCode");
}

#[test]
fn compiles_the_same_exists_semantics_to_oracle() {
  let statement = graph().compile_oracle(&operation()).unwrap();

  assert_eq!(
    statement.sql,
    concat!(
      "SELECT\n",
      "  \"t0\".\"id\" AS \"c0\"\n",
      "FROM \"Staff\" \"t0\"\n",
      "WHERE\n",
      "  EXISTS (\n",
      "    SELECT 1\n",
      "    FROM \"StaffRole\" \"t1\"\n",
      "    INNER JOIN \"AccessRole\" \"t2\"\n",
      "      ON (\"t1\".\"idRole\" = \"t2\".\"id\")\n",
      "    INNER JOIN \"AccessEventRole\" \"t3\"\n",
      "      ON (\"t2\".\"id\" = \"t3\".\"idAccessRole\")\n",
      "    INNER JOIN \"Event\" \"t4\"\n",
      "      ON (\"t3\".\"idEvent\" = \"t4\".\"id\")\n",
      "    WHERE (\"t0\".\"id\" = \"t1\".\"idStaff\")\n",
      "      AND (\"t4\".\"code\" = :p0)\n",
      "  )\n",
      "ORDER BY\n",
      "  \"t0\".\"id\" ASC\n",
      "OFFSET 0 ROWS FETCH NEXT 25 ROWS ONLY"
    )
  );
  assert!(statement.relations.is_empty());
  assert_eq!(statement.bindings[0].parameter, "eventCode");
}

#[test]
fn compiles_exists_without_a_predicate_and_not_exists_composition() {
  let mut definition = definition();
  definition.parameters.clear();
  definition.constraints[0].predicate = Expression::Not {
    expression: Box::new(Expression::exists("staffRole")),
  };
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();
  let statement = graph
    .compile_sql_server(&QueryOperation {
      limit: Some(5),
      ..QueryOperation::default()
    })
    .unwrap();

  assert!(statement.sql.contains(
    "(NOT EXISTS (\n    SELECT 1\n    FROM [StaffRole] AS [t1]\n    WHERE ([t0].[id] = [t1].[idStaff])\n  ))"
  ));
  assert!(statement.bindings.is_empty());
  assert!(statement.relations.is_empty());
}

#[test]
fn requires_optional_parameters_used_by_the_exists_relation_path() {
  let mut definition = definition();
  definition.parameters = vec![ParameterDefinition::optional(
    "relationStaffId",
    ScalarType::Int64,
  )];
  definition.relations[0].on = Expression::and([
    Expression::eq(
      Expression::field("staff", "id"),
      Expression::field("staffRole", "idStaff"),
    ),
    Expression::eq(
      Expression::field("staffRole", "idStaff"),
      Expression::parameter("relationStaffId"),
    ),
  ]);
  definition.constraints[0].predicate = Expression::exists("event");
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();

  let error = graph
    .compile_sql_server(&QueryOperation::default())
    .unwrap_err();
  let SqlCompileError::Plan(PlanError::Operation(issues)) = error else {
    panic!("expected operation validation error");
  };

  assert!(issues.as_slice().iter().any(|issue| {
    issue.code == OperationIssueCode::MissingParameter
      && issue.location == "parameters.relationStaffId"
  }));
}

#[test]
fn anchors_exists_to_an_outer_relation_source() {
  let mut definition = definition();
  definition.parameters.clear();
  definition.constraints[0].predicate = Expression::exists_from("accessRole", "staffRole");
  definition.projection.fields.push(
    ProjectionFieldDefinition::new(
      vec!["roleId".into()],
      Expression::field("staffRole", "idRole"),
    )
    .selected_by_default(),
  );
  let graph = MappedQueryGraph::new(definition.compile().unwrap(), mapping()).unwrap();
  let statement = graph.compile_oracle(&QueryOperation::default()).unwrap();

  assert!(statement.sql.contains(concat!(
    "JOIN \"StaffRole\" \"t1\"\n",
    "  ON (\"t0\".\"id\" = \"t1\".\"idStaff\")"
  )));
  assert!(statement.sql.contains(
    "EXISTS (\n    SELECT 1\n    FROM \"AccessRole\" \"t2\"\n    WHERE (\"t1\".\"idRole\" = \"t2\".\"id\")"
  ));
  assert_eq!(
    statement
      .relations
      .iter()
      .map(|relation| relation.name.as_str())
      .collect::<Vec<_>>(),
    vec!["staffRoles"]
  );
}
