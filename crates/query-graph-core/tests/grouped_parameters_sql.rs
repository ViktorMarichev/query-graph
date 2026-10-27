use std::collections::HashMap;

use query_graph_core::{
  BindingPathSegment::{Index, Property},
  ConstraintDefinition, Expression, FieldDefinition, GraphDefinition, MappedQueryGraph,
  OrderByDefinition, OrderingDefinition, ParameterDefinition, ProjectionFieldDefinition,
  QueryOperation, RelationDefinition, RelationalMapping, ScalarType, SourceDefinition,
  SourceMapping, SqlCompileError,
};
use serde_json::{json, Value};

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("itemsByCharacteristics", "items");
  definition.sources = vec![
    SourceDefinition::new("items", vec![FieldDefinition::new("id", ScalarType::Int64)]),
    SourceDefinition::new(
      "links",
      vec![
        FieldDefinition::new("ownerId", ScalarType::Int64),
        FieldDefinition::new("valueId", ScalarType::Int64),
        FieldDefinition::new("deletedAt", ScalarType::DateTime).nullable(),
      ],
    ),
    SourceDefinition::new(
      "values",
      vec![
        FieldDefinition::new("id", ScalarType::Int64),
        FieldDefinition::new("groupId", ScalarType::Int32),
      ],
    ),
  ];
  definition.parameters = vec![ParameterDefinition::optional_grouped_list(
    "characteristics",
    ScalarType::Int32,
    ScalarType::Int64,
  )];
  definition.relations = vec![
    RelationDefinition::new(
      "links",
      "items",
      "links",
      Expression::and([
        Expression::eq(
          Expression::field("items", "id"),
          Expression::field("links", "ownerId"),
        ),
        Expression::IsNull {
          expression: Box::new(Expression::field("links", "deletedAt")),
        },
      ]),
    )
    .many(),
    RelationDefinition::new(
      "value",
      "links",
      "values",
      Expression::eq(
        Expression::field("links", "valueId"),
        Expression::field("values", "id"),
      ),
    ),
  ];
  definition.constraints = vec![ConstraintDefinition::always(Expression::all_groups(
    "characteristics",
    "group",
    Expression::exists_where(
      "values",
      Expression::and([
        Expression::eq(
          Expression::field("values", "groupId"),
          Expression::group_key("group"),
        ),
        Expression::in_group_values(Expression::field("values", "id"), "group"),
      ]),
    ),
  ))];
  definition.projection.fields =
    vec![
      ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("items", "id"))
        .selected_by_default(),
    ];
  definition.orderings = vec![OrderingDefinition::new(
    "id",
    [OrderByDefinition::asc(Expression::field("items", "id"))],
  )
  .selected_by_default()];
  definition
}

fn mapped(definition: GraphDefinition) -> MappedQueryGraph {
  let sources = definition
    .sources
    .iter()
    .map(|source| (source.key.clone(), SourceMapping::new(source.key.clone())))
    .collect();
  MappedQueryGraph::new(definition.compile().unwrap(), RelationalMapping { sources }).unwrap()
}

fn operation(groups: Value) -> QueryOperation {
  QueryOperation {
    parameters: HashMap::from([("characteristics".into(), groups)]),
    limit: Some(20),
    ..QueryOperation::default()
  }
}

#[test]
fn correlates_each_group_without_materializing_many_relations() {
  let graph = mapped(definition());
  let operation = operation(json!([
    {"key": 12, "values": [101, 102]}, {"key": 14, "values": [201]},
  ]));
  let sql_server = graph.compile_sql_server(&operation).unwrap();
  let oracle = graph.compile_oracle(&operation).unwrap();
  for statement in [&sql_server, &oracle] {
    assert_eq!(statement.sql.matches("EXISTS (").count(), 2);
    assert_eq!(statement.sql.matches("IS NULL").count(), 2);
    assert!(statement.sql.contains(") AND EXISTS ("));
    assert!(statement.relations.is_empty());
    assert_eq!(statement.bindings.len(), 5);
    assert_eq!(statement.bindings[0].scalar_type, ScalarType::Int32);
    assert_eq!(statement.bindings[1].scalar_type, ScalarType::Int64);
    let paths: Vec<_> = statement
      .bindings
      .iter()
      .map(|binding| {
        assert_eq!(binding.parameter, "characteristics");
        assert_eq!(binding.index, None);
        binding.path.clone().unwrap()
      })
      .collect();
    assert_eq!(
      paths,
      vec![
        vec![Index(0), Property("key".into())],
        vec![Index(0), Property("values".into()), Index(0)],
        vec![Index(0), Property("values".into()), Index(1)],
        vec![Index(1), Property("key".into())],
        vec![Index(1), Property("values".into()), Index(0)],
      ]
    );
  }
  assert_eq!(
    sql_server.sql.matches("[t0].[id] = [t1].[ownerId]").count(),
    2
  );
  assert_eq!(
    oracle
      .sql
      .matches("\"t0\".\"id\" = \"t1\".\"ownerId\"")
      .count(),
    2
  );
  assert!(sql_server.sql.contains("[t2].[id] IN (@p1, @p2)"));
  assert!(oracle.sql.contains("\"t2\".\"id\" IN (:p1, :p2)"));
}

#[test]
fn repeated_keys_are_independent_conditions_and_empty_values_are_false() {
  let graph = mapped(definition());
  let op = operation(json!([
    {"key": 12, "values": [101]}, {"key": 12, "values": [102]},
  ]));
  let statement = graph.compile_sql_server(&op).unwrap();
  assert_eq!(statement.sql.matches("EXISTS (").count(), 2);
  assert_eq!(statement.bindings.len(), 4);

  let empty = operation(json!([{"key": 12, "values": []}]));
  for statement in [
    graph.compile_sql_server(&empty).unwrap(),
    graph.compile_oracle(&empty).unwrap(),
  ] {
    assert!(statement.sql.contains("(1 = 0)"));
    assert_eq!(statement.bindings.len(), 1);
    assert_eq!(
      statement.bindings[0].path,
      Some(vec![Index(0), Property("key".into())])
    );
  }
}

#[test]
fn inactive_templates_do_not_require_parameters_or_outer_joins() {
  let mut definition = definition();
  definition
    .parameters
    .push(ParameterDefinition::optional("probe", ScalarType::Int64));
  definition.constraints[0].predicate = Expression::all_groups(
    "characteristics",
    "group",
    Expression::eq(
      Expression::field("links", "valueId"),
      Expression::parameter("probe"),
    ),
  );
  let graph = mapped(definition);
  for operation in [QueryOperation::default(), operation(json!([]))] {
    for statement in [
      graph.compile_sql_server(&operation).unwrap(),
      graph.compile_oracle(&operation).unwrap(),
    ] {
      assert!(statement.relations.is_empty());
      assert!(statement.bindings.is_empty());
      assert!(statement.sql.contains("(1 = 1)"));
    }
  }
}

#[test]
fn nested_templates_capture_outer_keys_and_restore_the_outer_scope() {
  let mut definition = definition();
  definition
    .parameters
    .push(ParameterDefinition::optional_grouped_list(
      "inner",
      ScalarType::Int32,
      ScalarType::Int64,
    ));
  let key_predicate = Expression::eq(
    Expression::field("values", "groupId"),
    Expression::group_key("outer"),
  );
  definition.constraints[0].predicate = Expression::all_groups(
    "characteristics",
    "outer",
    Expression::and([
      Expression::all_groups(
        "inner",
        "innerScope",
        Expression::exists_where(
          "values",
          Expression::and([
            key_predicate.clone(),
            Expression::in_group_values(Expression::field("values", "id"), "innerScope"),
          ]),
        ),
      ),
      Expression::exists_where("values", key_predicate),
    ]),
  );
  let graph = mapped(definition);
  let mut operation = operation(json!([{"key": 12, "values": []}, {"key": 14, "values": []}]));
  operation
    .parameters
    .insert("inner".into(), json!([{"key": 0, "values": [101, 102]}]));
  let statement = graph.compile_sql_server(&operation).unwrap();
  assert_eq!(statement.sql.matches("EXISTS (").count(), 4);
  assert_eq!(statement.bindings.len(), 4);
  assert_eq!(
    statement.bindings[0].path,
    Some(vec![Index(0), Property("key".into())])
  );
  assert_eq!(statement.bindings[1].parameter, "inner");
  assert_eq!(
    statement.bindings[3].path,
    Some(vec![Index(1), Property("key".into())])
  );
}

#[test]
fn simultaneous_operations_do_not_mutate_the_registered_graph() {
  let definition = definition();
  let graph = mapped(definition.clone());
  std::thread::scope(|scope| {
    let handles: Vec<_> = (0..8)
      .map(|count| {
        let graph = &graph;
        scope.spawn(move || {
          let groups: Vec<_> = (0..count)
            .map(|index| json!({"key": index, "values": [index * 10]}))
            .collect();
          let statement = graph.compile_oracle(&operation(json!(groups))).unwrap();
          assert_eq!(statement.sql.matches("EXISTS (").count(), count);
          assert_eq!(statement.bindings.len(), count * 2);
        })
      })
      .collect();
    for handle in handles {
      handle.join().unwrap();
    }
  });
  assert_eq!(graph.graph().definition(), &definition);
  assert!(graph
    .compile_oracle(&QueryOperation::default())
    .unwrap()
    .bindings
    .is_empty());
}

#[test]
fn grouped_values_use_oracle_in_chunks_and_sql_server_total_binding_limit() {
  let graph = mapped(definition());
  let values: Vec<_> = (0..1001).collect();
  let statement = graph
    .compile_oracle(&operation(json!([{"key": 12, "values": values}])))
    .unwrap();
  assert_eq!(statement.sql.matches(" IN (").count(), 2);
  assert_eq!(statement.bindings.len(), 1002);
  assert_eq!(
    statement.bindings[1001].path,
    Some(vec![Index(0), Property("values".into()), Index(1000)])
  );

  let values: Vec<_> = (0..2099).collect();
  assert_eq!(
    graph
      .compile_sql_server(&operation(json!([{"key": 12, "values": values}])))
      .unwrap()
      .bindings
      .len(),
    2100
  );
  let values: Vec<_> = (0..2100).collect();
  assert!(matches!(
    graph.compile_sql_server(&operation(json!([{"key": 12, "values": values}]))),
    Err(SqlCompileError::TooManyParameters { limit: 2100, .. })
  ));
}

#[test]
fn preserves_first_by_and_parameters_inside_each_exists_path() {
  let mut definition = definition();
  definition
    .parameters
    .push(ParameterDefinition::optional("probe", ScalarType::Int64));
  let first = &mut definition.relations[0];
  first.cardinality = query_graph_core::RelationCardinality::One;
  first.on = Expression::and([
    first.on.clone(),
    Expression::eq(
      Expression::field("links", "valueId"),
      Expression::parameter("probe"),
    ),
  ]);
  *first = first
    .clone()
    .first_by([OrderByDefinition::asc(Expression::field(
      "links", "valueId",
    ))]);
  let graph = mapped(definition);
  assert!(graph
    .compile_oracle(&QueryOperation::default())
    .unwrap()
    .bindings
    .is_empty());
  let mut op = operation(json!([{"key": 12, "values": [101]}, {"key": 14, "values": [201]}]));
  assert!(graph.compile_oracle(&op).is_err());
  op.parameters.insert("probe".into(), json!(101));
  for statement in [
    graph.compile_oracle(&op).unwrap(),
    graph.compile_sql_server(&op).unwrap(),
  ] {
    assert_eq!(statement.sql.matches("CROSS APPLY").count(), 2);
    assert_eq!(statement.bindings.len(), 5);
    assert_eq!(statement.bindings[0].parameter, "probe");
    assert_eq!(statement.bindings[0].index, None);
    assert_eq!(statement.bindings[0].path, None);
    assert!(statement.relations.is_empty());
  }
}

#[test]
fn bounds_nested_expansion_even_when_bindings_are_reused() {
  let mut definition = definition();
  definition.constraints[0].predicate = Expression::all_groups(
    "characteristics",
    "outer",
    Expression::all_groups(
      "characteristics",
      "middle",
      Expression::all_groups(
        "characteristics",
        "inner",
        Expression::eq(
          Expression::field("items", "id"),
          Expression::group_key("inner"),
        ),
      ),
    ),
  );
  let graph = mapped(definition);
  let groups: Vec<_> = (0..30)
    .map(|key| json!({"key": key, "values": []}))
    .collect();
  let too_large = operation(json!(groups));
  for result in [
    graph.compile_oracle(&too_large),
    graph.compile_sql_server(&too_large),
  ] {
    assert!(matches!(
      result,
      Err(SqlCompileError::GroupExpansionLimitExceeded { limit: 10_000 })
    ));
  }
  let small = operation(json!([{"key": 1, "values": []}]));
  assert_eq!(graph.compile_oracle(&small).unwrap().bindings.len(), 1);
}

#[test]
fn bounds_repeated_list_expansion_with_few_groups_and_bindings() {
  for captured in [false, true] {
    let mut definition = definition();
    definition
      .parameters
      .push(ParameterDefinition::optional_list("ids", ScalarType::Int64));
    definition
      .parameters
      .push(ParameterDefinition::optional_grouped_list(
        "inner",
        ScalarType::Int32,
        ScalarType::Int64,
      ));
    let membership = if captured {
      Expression::in_group_values(Expression::field("items", "id"), "outer")
    } else {
      Expression::in_parameter(Expression::field("items", "id"), "ids")
    };
    definition.constraints[0].predicate = Expression::all_groups(
      "characteristics",
      "outer",
      Expression::all_groups("inner", "innerScope", membership),
    );
    let graph = mapped(definition);
    let values: Vec<_> = (0..1000).collect();
    let mut op = operation(json!([{"key": 1, "values": values}]));
    op.parameters.insert("ids".into(), json!(values));
    op.parameters.insert(
      "inner".into(),
      json!(vec![json!({"key": 1, "values": []}); 101]),
    );
    for result in [graph.compile_oracle(&op), graph.compile_sql_server(&op)] {
      assert!(matches!(
        result,
        Err(SqlCompileError::GroupValueExpansionLimitExceeded { limit: 100_000 })
      ));
    }
    op.parameters.insert(
      "inner".into(),
      json!(vec![json!({"key": 1, "values": []}); 100]),
    );
    assert_eq!(graph.compile_oracle(&op).unwrap().bindings.len(), 1000);
    assert_eq!(graph.compile_sql_server(&op).unwrap().bindings.len(), 1000);
  }
}
