use query_graph_core::{
  ConstraintDefinition, DefinitionIssueCode, Expression, ExpressionType, FieldDefinition,
  GraphDefinition, LiteralValue, OrderByDefinition, OrderingDefinition, ProjectionDefinition,
  ProjectionFieldDefinition, RelationDefinition, ScalarType, SemanticFunction, SourceDefinition,
};

fn root_source() -> SourceDefinition {
  SourceDefinition::new(
    "root",
    vec![
      FieldDefinition::new("id", ScalarType::Int64),
      FieldDefinition::new("quantity", ScalarType::Int32),
      FieldDefinition::new("name", ScalarType::String),
      FieldDefinition::new("optionalName", ScalarType::String).nullable(),
      FieldDefinition::new("active", ScalarType::Boolean),
      FieldDefinition::new("payload", ScalarType::Json).nullable(),
    ],
  )
}

fn definition() -> GraphDefinition {
  let mut definition = GraphDefinition::new("typedGraph", "root");
  definition.sources = vec![root_source()];
  definition
}

fn issue_codes(definition: GraphDefinition) -> Vec<DefinitionIssueCode> {
  definition
    .compile()
    .unwrap_err()
    .into_vec()
    .into_iter()
    .map(|issue| issue.code)
    .collect()
}

#[test]
fn infers_projection_scalar_types_and_nullability() {
  let mut definition = definition();
  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(vec!["id".into()], Expression::field("root", "id")),
      ProjectionFieldDefinition::new(
        vec!["normalizedName".into()],
        Expression::Function {
          name: SemanticFunction::Lower,
          arguments: vec![Expression::field("root", "optionalName")],
        },
      ),
      ProjectionFieldDefinition::new(
        vec!["displayName".into()],
        Expression::Function {
          name: SemanticFunction::Coalesce,
          arguments: vec![
            Expression::field("root", "optionalName"),
            Expression::literal(LiteralValue::String("Unknown".into())),
          ],
        },
      ),
      ProjectionFieldDefinition::new(
        vec!["hasOne".into()],
        Expression::eq(
          Expression::field("root", "quantity"),
          Expression::literal(LiteralValue::Integer(1)),
        ),
      ),
    ],
  };

  let graph = definition.compile().unwrap();

  assert_eq!(
    graph.projection_type("id"),
    Some(ExpressionType::new(ScalarType::Int64, false))
  );
  assert_eq!(
    graph.projection_type("normalizedName"),
    Some(ExpressionType::new(ScalarType::String, true))
  );
  assert_eq!(
    graph.projection_type("displayName"),
    Some(ExpressionType::new(ScalarType::String, false))
  );
  assert_eq!(
    graph.projection_type("hasOne"),
    Some(ExpressionType::new(ScalarType::Boolean, false))
  );
}

#[test]
fn propagates_optional_relation_nullability_into_projection_types() {
  let mut definition = definition();
  definition.sources.push(SourceDefinition::new(
    "child",
    vec![
      FieldDefinition::new("idRoot", ScalarType::Int64),
      FieldDefinition::new("name", ScalarType::String),
    ],
  ));
  definition.relations = vec![RelationDefinition::new(
    "child",
    "root",
    "child",
    Expression::eq(
      Expression::field("root", "id"),
      Expression::field("child", "idRoot"),
    ),
  )];
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["child".into(), "name".into()],
      Expression::field("child", "name"),
    )],
  };

  let graph = definition.compile().unwrap();

  assert_eq!(
    graph.projection_type("child.name"),
    Some(ExpressionType::new(ScalarType::String, true))
  );
}

#[test]
fn rejects_incompatible_comparison_types() {
  let mut definition = definition();
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["invalid".into()],
      Expression::eq(
        Expression::field("root", "id"),
        Expression::field("root", "name"),
      ),
    )],
  };

  assert!(issue_codes(definition).contains(&DefinitionIssueCode::IncompatibleExpressionTypes));
}

#[test]
fn requires_boolean_relation_and_constraint_predicates() {
  let mut definition = definition();
  definition.sources.push(SourceDefinition::new(
    "child",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  ));
  definition.relations = vec![RelationDefinition::new(
    "child",
    "root",
    "child",
    Expression::field("root", "id"),
  )];
  definition.constraints = vec![ConstraintDefinition::always(Expression::field(
    "root", "name",
  ))];

  let codes = issue_codes(definition);

  assert_eq!(
    codes
      .iter()
      .filter(|code| **code == DefinitionIssueCode::InvalidPredicateType)
      .count(),
    2
  );
}

#[test]
fn validates_semantic_function_arities_and_arguments() {
  let mut definition = definition();
  definition.projection = ProjectionDefinition {
    fields: vec![
      ProjectionFieldDefinition::new(
        vec!["wrongArity".into()],
        Expression::Function {
          name: SemanticFunction::Lower,
          arguments: vec![
            Expression::field("root", "name"),
            Expression::field("root", "name"),
          ],
        },
      ),
      ProjectionFieldDefinition::new(
        vec!["wrongType".into()],
        Expression::Function {
          name: SemanticFunction::Upper,
          arguments: vec![Expression::field("root", "id")],
        },
      ),
    ],
  };

  let codes = issue_codes(definition);

  assert!(codes.contains(&DefinitionIssueCode::InvalidFunctionArity));
  assert!(codes.contains(&DefinitionIssueCode::InvalidExpressionType));
}

#[test]
fn rejects_untyped_projections_and_non_orderable_values() {
  let mut definition = definition();
  definition.projection = ProjectionDefinition {
    fields: vec![ProjectionFieldDefinition::new(
      vec!["nothing".into()],
      Expression::literal(LiteralValue::Null),
    )],
  };
  definition.orderings = vec![OrderingDefinition::new(
    "default",
    [OrderByDefinition::asc(Expression::field("root", "payload"))],
  )
  .selected_by_default()];

  let codes = issue_codes(definition);

  assert!(codes.contains(&DefinitionIssueCode::UnresolvedExpressionType));
  assert!(codes.contains(&DefinitionIssueCode::InvalidOrderExpression));
}
