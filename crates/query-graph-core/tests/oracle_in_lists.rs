use std::collections::HashMap;

use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, FieldDefinition, GraphDefinition,
  LiteralValue, MappedQueryGraph, OperationIssueCode, OracleCompiler, OracleVersion,
  OrderByDefinition, OrderingDefinition, ParameterDefinition, PlanError, ProjectionDefinition,
  ProjectionFieldDefinition, QueryOperation, RelationalMapping, ScalarType, SourceDefinition,
  SourceMapping, SqlCompileError,
};
use serde_json::json;

fn id() -> Expression {
  Expression::field("rows", "id")
}

fn definition(predicate: Expression, parameters: Vec<ParameterDefinition>) -> GraphDefinition {
  let mut definition = GraphDefinition::new("rowsByIds", "rows");
  definition.sources = vec![SourceDefinition::new(
    "rows",
    vec![FieldDefinition::new("id", ScalarType::Int64).nullable()],
  )];
  definition.parameters = parameters;
  definition.constraints = vec![ConstraintDefinition::always(predicate)];
  definition.projection = ProjectionDefinition {
    objects: Vec::new(),
    fields: vec![ProjectionFieldDefinition::new(vec!["id".into()], id()).selected_by_default()],
  };
  definition
}

fn graph(definition: GraphDefinition) -> MappedQueryGraph {
  MappedQueryGraph::new(
    definition.compile().unwrap(),
    RelationalMapping {
      sources: HashMap::from([("rows".into(), SourceMapping::new("Rows"))]),
    },
  )
  .unwrap()
}

fn operation(count: usize) -> QueryOperation {
  QueryOperation {
    parameters: HashMap::from([("ids".into(), json!((0..count).collect::<Vec<_>>()))]),
    ..QueryOperation::default()
  }
}

fn in_list_sizes(sql: &str) -> Vec<usize> {
  sql
    .split(" IN (")
    .skip(1)
    .map(|tail| tail.split_once(')').unwrap().0.split(", ").count())
    .collect()
}

fn negate(expression: Expression, negated: bool) -> Expression {
  if negated {
    Expression::Not {
      expression: Box::new(expression),
    }
  } else {
    expression
  }
}

fn literal_list(count: usize) -> Expression {
  Expression::In {
    expression: Box::new(id()),
    values: (0..count)
      .map(|value| Expression::literal(LiteralValue::Integer(value as i64)))
      .collect(),
  }
}

fn predicate(sql: &str) -> &str {
  sql.split_once("WHERE\n  ").unwrap().1
}

const ORACLE_VERSIONS: [OracleVersion; 5] = [
  OracleVersion::V11g,
  OracleVersion::V12c,
  OracleVersion::V19c,
  OracleVersion::V21c,
  OracleVersion::V23ai,
];

#[test]
fn oracle_19c_splits_1001_list_bindings() {
  let graph = graph(definition(
    Expression::in_parameter(id(), "ids"),
    vec![ParameterDefinition::required_list("ids", ScalarType::Int64)],
  ));
  let statement = graph
    .compile_oracle_with(&operation(1001), &OracleCompiler::new(OracleVersion::V19c))
    .unwrap();

  assert_eq!(in_list_sizes(&statement.sql), vec![1000, 1]);
}

#[test]
fn list_parameter_boundaries_preserve_bindings_and_negation_for_each_oracle_version() {
  for version in ORACLE_VERSIONS {
    for (count, legacy_sizes) in [
      (0, vec![]),
      (1, vec![1]),
      (1000, vec![1000]),
      (1001, vec![1000, 1]),
      (2001, vec![1000, 1000, 1]),
    ] {
      let mut positive_predicate = String::new();
      for negated in [false, true] {
        let graph = graph(definition(
          negate(Expression::in_parameter(id(), "ids"), negated),
          vec![ParameterDefinition::required_list("ids", ScalarType::Int64)],
        ));
        let statement = graph
          .compile_oracle_with(&operation(count), &OracleCompiler::new(version))
          .unwrap();
        let sizes = if version == OracleVersion::V23ai && count > 0 {
          vec![count]
        } else {
          legacy_sizes.clone()
        };
        assert_eq!(in_list_sizes(&statement.sql), sizes, "{version:?}, {count}");
        assert_eq!(statement.bindings.len(), count);
        for (index, binding) in statement.bindings.iter().enumerate() {
          assert_eq!(binding.name, format!("p{index}"));
          assert_eq!(binding.parameter, "ids");
          assert_eq!(binding.index, Some(index));
          assert_eq!(binding.scalar_type, ScalarType::Int64);
        }
        if negated {
          assert_eq!(
            predicate(&statement.sql),
            format!("(NOT {positive_predicate})")
          );
        } else {
          positive_predicate = predicate(&statement.sql).to_owned();
          if count == 0 {
            assert_eq!(positive_predicate, "(1 = 0)");
          } else {
            assert!(positive_predicate.contains(":p0"));
            assert!(positive_predicate.ends_with(if sizes.len() > 1 { ")))" } else { "))" }));
            if sizes.len() > 1 {
              assert!(positive_predicate.starts_with("((\"t0\".\"id\" IN ("));
              assert!(positive_predicate.contains(")) OR (\"t0\".\"id\" IN (:p1000"));
            }
          }
        }
      }
    }
  }
}

#[test]
fn literal_boundaries_preserve_order_duplicates_zero_and_negation() {
  for version in ORACLE_VERSIONS {
    for (count, legacy_sizes) in [
      (1, vec![1]),
      (1000, vec![1000]),
      (1001, vec![1000, 1]),
      (2001, vec![1000, 1000, 1]),
    ] {
      let mut positive_predicate = String::new();
      for negated in [false, true] {
        let mut membership = literal_list(count);
        if let Expression::In { values, .. } = &mut membership {
          values[count - 1] = Expression::literal(LiteralValue::Integer(0));
        }
        let statement = graph(definition(negate(membership, negated), vec![]))
          .compile_oracle_with(&QueryOperation::default(), &OracleCompiler::new(version))
          .unwrap();
        let sizes = if version == OracleVersion::V23ai {
          vec![count]
        } else {
          legacy_sizes.clone()
        };
        assert_eq!(in_list_sizes(&statement.sql), sizes, "{version:?}, {count}");
        assert!(statement.bindings.is_empty());
        if negated {
          assert_eq!(
            predicate(&statement.sql),
            format!("(NOT {positive_predicate})")
          );
        } else {
          positive_predicate = predicate(&statement.sql).to_owned();
          let values = statement
            .sql
            .split(" IN (")
            .skip(1)
            .flat_map(|tail| tail.split_once(')').unwrap().0.split(", "))
            .collect::<Vec<_>>();
          let mut expected = (0..count)
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
          expected[count - 1] = "0".to_owned();
          assert_eq!(values, expected);
        }
      }
    }
  }
}

#[test]
fn empty_literal_lists_remain_invalid_under_negation() {
  for negated in [false, true] {
    let issues = definition(negate(literal_list(0), negated), vec![])
      .compile()
      .unwrap_err();
    assert!(issues
      .as_slice()
      .iter()
      .any(|issue| issue.code == DefinitionIssueCode::EmptyExpressionGroup));
  }
}

#[test]
fn oracle_23ai_uses_its_65535_expression_limit_for_both_list_paths() {
  let parameter_graph = graph(definition(
    Expression::in_parameter(id(), "ids"),
    vec![ParameterDefinition::required_list("ids", ScalarType::Int64)],
  ));
  for (count, sizes) in [(65_535, vec![65_535]), (65_536, vec![65_535, 1])] {
    let compiler = OracleCompiler::new(OracleVersion::V23ai);
    let parameter_statement = parameter_graph
      .compile_oracle_with(&operation(count), &compiler)
      .unwrap();
    let literal_statement = graph(definition(literal_list(count), vec![]))
      .compile_oracle_with(&QueryOperation::default(), &compiler)
      .unwrap();
    assert_eq!(in_list_sizes(&parameter_statement.sql), sizes);
    assert_eq!(in_list_sizes(&literal_statement.sql), sizes);
    assert_eq!(parameter_statement.bindings.len(), count);
    assert_eq!(
      parameter_statement.bindings[count - 1].index,
      Some(count - 1)
    );
  }
}

#[test]
fn nested_lists_reuse_bindings_and_preserve_constraints_ordering_and_pagination() {
  let membership = Expression::in_parameter(id(), "ids");
  let other_membership = Expression::in_parameter(id(), "otherIds");
  let guard = Expression::eq(id(), Expression::literal(LiteralValue::Integer(0)));
  let mut definition = definition(
    Expression::and([
      Expression::or([membership.clone(), guard.clone()]),
      negate(other_membership, true),
      membership,
    ]),
    vec![
      ParameterDefinition::required_list("ids", ScalarType::Int64),
      ParameterDefinition::required_list("otherIds", ScalarType::Int32),
    ],
  );
  definition
    .constraints
    .push(ConstraintDefinition::always(guard));
  definition.orderings =
    vec![OrderingDefinition::new("idDesc", [OrderByDefinition::desc(id())]).selected_by_default()];
  let graph = graph(definition);
  let mut operation = operation(2001);
  operation.parameters.get_mut("ids").unwrap()[1000] = json!(0);
  operation
    .parameters
    .insert("otherIds".into(), json!(vec![0; 1001]));
  operation.offset = Some(25);
  operation.limit = Some(10);
  let statement = graph.compile_oracle(&operation).unwrap();

  assert_eq!(
    in_list_sizes(&statement.sql),
    vec![1000, 1000, 1, 1000, 1, 1000, 1000, 1]
  );
  assert_eq!(statement.bindings.len(), 3002);
  for (binding_index, binding) in statement.bindings.iter().enumerate() {
    assert_eq!(binding.name, format!("p{binding_index}"));
    if binding_index < 2001 {
      assert_eq!(binding.parameter, "ids");
      assert_eq!(binding.index, Some(binding_index));
      assert_eq!(binding.scalar_type, ScalarType::Int64);
    } else {
      assert_eq!(binding.parameter, "otherIds");
      assert_eq!(binding.index, Some(binding_index - 2001));
      assert_eq!(binding.scalar_type, ScalarType::Int32);
    }
  }
  assert_eq!(statement.sql.matches("IN (:p0, ").count(), 2);
  assert_eq!(statement.sql.matches("IN (:p1000, ").count(), 2);
  assert_eq!(statement.sql.matches("IN (:p2000)").count(), 2);
  assert!(statement
    .sql
    .contains("IN (:p2000))) OR (\"t0\".\"id\" = 0)) AND (NOT (("));
  assert!(statement
    .sql
    .contains("IN (:p3001)))) AND ((\"t0\".\"id\" IN (:p0, "));
  assert!(statement.sql.ends_with(
    "\n  AND (\"t0\".\"id\" = 0)\nORDER BY\n  \"t0\".\"id\" DESC\nOFFSET 25 ROWS FETCH NEXT 10 ROWS ONLY"
  ));
  assert_eq!(statement.columns.len(), 1);
  assert_eq!(statement.columns[0].path, "id");
}

#[test]
fn scalar_left_operand_binding_is_reused_across_chunks_and_later_constraints() {
  let mut definition = definition(
    Expression::in_parameter(Expression::parameter("probe"), "ids"),
    vec![
      ParameterDefinition::required("probe", ScalarType::Int64),
      ParameterDefinition::required_list("ids", ScalarType::Int64),
    ],
  );
  definition
    .constraints
    .push(ConstraintDefinition::always(Expression::eq(
      id(),
      Expression::parameter("probe"),
    )));
  let graph = graph(definition);
  for count in [0, 1001, 2001] {
    let mut operation = operation(count);
    operation.parameters.insert("probe".into(), json!(0));
    let statement = graph.compile_oracle(&operation).unwrap();
    assert_eq!(statement.bindings.len(), count + 1);
    assert_eq!(statement.bindings[0].parameter, "probe");
    assert_eq!(statement.bindings[0].index, None);
    assert_eq!(statement.bindings[0].scalar_type, ScalarType::Int64);
    assert_eq!(
      statement.sql.matches(":p0").count(),
      count.div_ceil(1000) + 1
    );
    for (index, binding) in statement.bindings[1..].iter().enumerate() {
      assert_eq!(binding.name, format!("p{}", index + 1));
      assert_eq!(binding.parameter, "ids");
      assert_eq!(binding.index, Some(index));
      assert_eq!(binding.scalar_type, ScalarType::Int64);
    }
  }
}

#[test]
fn null_literal_and_nullable_probe_keep_three_valued_negation() {
  let mut membership = literal_list(1001);
  if let Expression::In { values, .. } = &mut membership {
    values[1000] = Expression::literal(LiteralValue::Null);
  }
  let membership = negate(membership, true);
  let mut definition = definition(membership.clone(), vec![]);
  definition.projection.fields.push(
    ProjectionFieldDefinition::new(vec!["excluded".into()], membership).selected_by_default(),
  );
  let statement = graph(definition)
    .compile_oracle(&QueryOperation::default())
    .unwrap();
  let expected = format!(
    "(NOT ((\"t0\".\"id\" IN ({})) OR (\"t0\".\"id\" IN (NULL))))",
    (0..1000)
      .map(|value| value.to_string())
      .collect::<Vec<_>>()
      .join(", ")
  );
  assert_eq!(predicate(&statement.sql), expected);
  assert!(statement.sql.contains(&format!(
    "CASE WHEN {expected} THEN 1 WHEN NOT ({expected}) THEN 0 ELSE NULL END"
  )));
  assert_eq!(statement.columns[1].scalar_type, ScalarType::Boolean);
  assert!(statement.columns[1].nullable);
  assert!(statement.bindings.is_empty());
}

#[test]
fn null_list_elements_remain_invalid_even_after_the_first_chunk() {
  let graph = graph(definition(
    Expression::in_parameter(id(), "ids"),
    vec![ParameterDefinition::required_list("ids", ScalarType::Int64)],
  ));
  let mut operation = operation(1001);
  operation.parameters.get_mut("ids").unwrap()[1000] = json!(null);
  let SqlCompileError::Plan(PlanError::Operation(issues)) =
    graph.compile_oracle(&operation).unwrap_err()
  else {
    panic!("expected operation validation error");
  };
  assert_eq!(issues.as_slice().len(), 1);
  assert_eq!(
    issues.as_slice()[0].code,
    OperationIssueCode::InvalidParameterType
  );
  assert_eq!(issues.as_slice()[0].location, "parameters.ids[1000]");
}

#[test]
fn sql_server_keeps_large_lists_in_one_membership_expression() {
  for negated in [false, true] {
    let parameter_statement = graph(definition(
      negate(Expression::in_parameter(id(), "ids"), negated),
      vec![ParameterDefinition::required_list("ids", ScalarType::Int64)],
    ))
    .compile_sql_server(&operation(2001))
    .unwrap();
    let literal_statement = graph(definition(negate(literal_list(2001), negated), vec![]))
      .compile_sql_server(&QueryOperation::default())
      .unwrap();
    assert_eq!(in_list_sizes(&parameter_statement.sql), vec![2001]);
    assert_eq!(in_list_sizes(&literal_statement.sql), vec![2001]);
    assert_eq!(parameter_statement.bindings.len(), 2001);
    assert!(!parameter_statement.sql.contains(" OR "));
    assert!(!literal_statement.sql.contains(" OR "));
  }
}
