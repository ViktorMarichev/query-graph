use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
  type_system::{self, InferredType, TypeSystemError, TypeSystemErrorKind},
  DefinitionIssue, DefinitionIssueCode, DefinitionIssues, Expression, ExpressionType,
  GraphDefinition, ParameterShape, ProjectionFieldRole,
};

pub(crate) fn analyze(
  definition: &GraphDefinition,
) -> Result<Vec<ExpressionType>, DefinitionIssues> {
  let source_nullability = infer_source_nullability(definition);
  let sources = definition
    .sources
    .iter()
    .map(|source| {
      let outer_nullable = source_nullability
        .get(source.key.as_str())
        .copied()
        .unwrap_or(false);
      let fields = source
        .fields
        .iter()
        .map(|field| {
          (
            field.name.as_str(),
            InferredType::scalar(field.scalar_type, field.nullable || outer_nullable),
          )
        })
        .collect();
      (source.key.as_str(), fields)
    })
    .collect();
  let parameters = definition
    .parameters
    .iter()
    .map(|parameter| {
      (
        parameter.name.as_str(),
        ParameterType {
          inferred: InferredType::parameter(parameter.scalar_type),
          shape: parameter.shape,
        },
      )
    })
    .collect();
  let environment = TypeEnvironment {
    sources,
    parameters,
  };
  let mut issues = Vec::new();

  for (index, relation) in definition.relations.iter().enumerate() {
    let location = format!("relations[{index}].on");
    let expression_type = infer_expression(&relation.on, &location, &environment, &mut issues);
    validate_predicate(expression_type, &location, &mut issues);
  }

  for (relation_index, relation) in definition.relations.iter().enumerate() {
    let Some(selection) = &relation.selection else {
      continue;
    };

    for (order_index, order) in selection.order_by().iter().enumerate() {
      let location =
        format!("relations[{relation_index}].selection.orderBy[{order_index}].expression");
      let expression_type =
        infer_expression(&order.expression, &location, &environment, &mut issues);
      if let Some(expression_type) = expression_type {
        if let Err(error) = type_system::require_orderable(expression_type) {
          issues.push(DefinitionIssue::new(
            DefinitionIssueCode::InvalidOrderExpression,
            location,
            error.message,
          ));
        }
      }
    }
  }

  for (index, constraint) in definition.constraints.iter().enumerate() {
    let location = format!("constraints[{index}].predicate");
    let expression_type =
      infer_expression(&constraint.predicate, &location, &environment, &mut issues);
    validate_predicate(expression_type, &location, &mut issues);
  }

  let mut projection_types = Vec::with_capacity(definition.projection.fields.len());
  for (index, projection) in definition.projection.fields.iter().enumerate() {
    let location = format!("projection.fields[{index}].expression");
    let expression_type =
      infer_expression(&projection.expression, &location, &environment, &mut issues);
    if projection.role == ProjectionFieldRole::Dimension {
      if let Some(expression_type) = expression_type {
        if let Err(error) = type_system::require_groupable(expression_type) {
          issues.push(DefinitionIssue::new(
            DefinitionIssueCode::InvalidDimensionExpression,
            &location,
            error.message,
          ));
        }
      }
    }
    let resolved = expression_type.and_then(|expression_type| {
      report_type_result(
        type_system::resolve_expression_type(expression_type),
        &location,
        &mut issues,
      )
    });
    projection_types.push(resolved);
  }

  for (index, order) in definition.default_order_by.iter().enumerate() {
    let location = format!("defaultOrderBy[{index}].expression");
    let expression_type = infer_expression(&order.expression, &location, &environment, &mut issues);
    if let Some(expression_type) = expression_type {
      if let Err(error) = type_system::require_orderable(expression_type) {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::InvalidOrderExpression,
          location,
          error.message,
        ));
      }
    }
  }

  if issues.is_empty() {
    Ok(
      projection_types
        .into_iter()
        .map(|expression_type| {
          expression_type.expect("valid projection expression must have a concrete type")
        })
        .collect(),
    )
  } else {
    Err(DefinitionIssues::from_vec(issues))
  }
}

type SourceTypes<'a> = HashMap<&'a str, HashMap<&'a str, InferredType>>;
type ParameterTypes<'a> = HashMap<&'a str, ParameterType>;

#[derive(Clone, Copy)]
struct ParameterType {
  inferred: InferredType,
  shape: ParameterShape,
}

struct TypeEnvironment<'a> {
  sources: SourceTypes<'a>,
  parameters: ParameterTypes<'a>,
}

fn infer_expression(
  expression: &Expression,
  location: &str,
  environment: &TypeEnvironment<'_>,
  issues: &mut Vec<DefinitionIssue>,
) -> Option<InferredType> {
  match expression {
    Expression::Field { source, field } => Some(
      *environment
        .sources
        .get(source.as_str())
        .and_then(|fields| fields.get(field.as_str()))
        .expect("structural validation must resolve expression fields"),
    ),
    Expression::Parameter { name } => environment
      .parameters
      .get(name.as_str())
      .filter(|parameter| parameter.shape == ParameterShape::Scalar)
      .map(|parameter| parameter.inferred),
    Expression::Literal { value } => Some(InferredType::literal(value)),
    Expression::Eq { left, right } | Expression::NotEq { left, right } => infer_binary(
      left,
      right,
      location,
      environment,
      issues,
      type_system::infer_equality,
    ),
    Expression::LessThan { left, right }
    | Expression::LessThanOrEqual { left, right }
    | Expression::GreaterThan { left, right }
    | Expression::GreaterThanOrEqual { left, right } => infer_binary(
      left,
      right,
      location,
      environment,
      issues,
      type_system::infer_ordering_comparison,
    ),
    Expression::Like {
      expression,
      pattern,
    } => {
      let expression_type = infer_expression(
        expression,
        &format!("{location}.expression"),
        environment,
        issues,
      );
      let pattern_type =
        infer_expression(pattern, &format!("{location}.pattern"), environment, issues);
      match (expression_type, pattern_type) {
        (Some(expression_type), Some(pattern_type)) => report_type_result(
          type_system::infer_like(expression_type, pattern_type),
          location,
          issues,
        ),
        _ => None,
      }
    }
    Expression::In { expression, values } => {
      let expression_type = infer_expression(
        expression,
        &format!("{location}.expression"),
        environment,
        issues,
      );
      let value_types =
        infer_expressions(values, &format!("{location}.values"), environment, issues);
      match (expression_type, value_types) {
        (Some(expression_type), Some(value_types)) => report_type_result(
          type_system::infer_in(expression_type, &value_types),
          location,
          issues,
        ),
        _ => None,
      }
    }
    Expression::InParameter {
      expression,
      parameter,
    } => {
      let expression_type = infer_expression(
        expression,
        &format!("{location}.expression"),
        environment,
        issues,
      );
      let parameter_type = environment
        .parameters
        .get(parameter.as_str())
        .filter(|parameter| parameter.shape == ParameterShape::List)
        .map(|parameter| parameter.inferred);
      match (expression_type, parameter_type) {
        (Some(expression_type), Some(parameter_type)) => report_type_result(
          type_system::infer_in(expression_type, &[parameter_type]),
          location,
          issues,
        ),
        _ => None,
      }
    }
    Expression::And { expressions } | Expression::Or { expressions } => {
      let expression_types = infer_expressions(
        expressions,
        &format!("{location}.expressions"),
        environment,
        issues,
      );
      expression_types.and_then(|expression_types| {
        report_type_result(
          type_system::infer_logical(&expression_types),
          location,
          issues,
        )
      })
    }
    Expression::Not { expression } => {
      let expression_type = infer_expression(
        expression,
        &format!("{location}.expression"),
        environment,
        issues,
      );
      expression_type.and_then(|expression_type| {
        report_type_result(type_system::infer_not(expression_type), location, issues)
      })
    }
    Expression::IsNull { expression } | Expression::IsNotNull { expression } => {
      let expression_type = infer_expression(
        expression,
        &format!("{location}.expression"),
        environment,
        issues,
      );
      expression_type.and_then(|expression_type| {
        report_type_result(
          type_system::infer_null_test(expression_type),
          location,
          issues,
        )
      })
    }
    Expression::Exists { predicate, .. } => {
      if let Some(predicate) = predicate {
        let predicate_location = format!("{location}.predicate");
        let predicate_type = infer_expression(predicate, &predicate_location, environment, issues);
        validate_predicate(predicate_type, &predicate_location, issues);
      }

      Some(InferredType::scalar(crate::ScalarType::Boolean, false))
    }
    Expression::Function { name, arguments } => {
      let argument_types = infer_expressions(
        arguments,
        &format!("{location}.arguments"),
        environment,
        issues,
      );
      argument_types.and_then(|argument_types| {
        report_type_result(
          type_system::infer_function(*name, &argument_types),
          location,
          issues,
        )
      })
    }
    Expression::Aggregate {
      function,
      expression,
    } => {
      let expression_type = match expression.as_deref() {
        Some(expression) => Some(infer_expression(
          expression,
          &format!("{location}.expression"),
          environment,
          issues,
        )?),
        None => None,
      };
      report_type_result(
        type_system::infer_aggregate(*function, expression_type),
        location,
        issues,
      )
    }
  }
}

fn infer_binary(
  left: &Expression,
  right: &Expression,
  location: &str,
  environment: &TypeEnvironment<'_>,
  issues: &mut Vec<DefinitionIssue>,
  infer: impl FnOnce(InferredType, InferredType) -> Result<InferredType, TypeSystemError>,
) -> Option<InferredType> {
  let left_type = infer_expression(left, &format!("{location}.left"), environment, issues);
  let right_type = infer_expression(right, &format!("{location}.right"), environment, issues);
  match (left_type, right_type) {
    (Some(left_type), Some(right_type)) => {
      report_type_result(infer(left_type, right_type), location, issues)
    }
    _ => None,
  }
}

fn infer_expressions(
  expressions: &[Expression],
  location: &str,
  environment: &TypeEnvironment<'_>,
  issues: &mut Vec<DefinitionIssue>,
) -> Option<Vec<InferredType>> {
  let mut all_valid = true;
  let types = expressions
    .iter()
    .enumerate()
    .filter_map(|(index, expression)| {
      let expression_type = infer_expression(
        expression,
        &format!("{location}[{index}]"),
        environment,
        issues,
      );
      all_valid &= expression_type.is_some();
      expression_type
    })
    .collect();
  all_valid.then_some(types)
}

fn validate_predicate(
  expression_type: Option<InferredType>,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
  let Some(expression_type) = expression_type else {
    return;
  };

  if let Err(error) = type_system::require_predicate(expression_type) {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidPredicateType,
      location,
      error.message,
    ));
  }
}

fn report_type_result<T>(
  result: Result<T, TypeSystemError>,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) -> Option<T> {
  match result {
    Ok(value) => Some(value),
    Err(error) => {
      issues.push(DefinitionIssue::new(
        issue_code(error.kind),
        location,
        error.message,
      ));
      None
    }
  }
}

fn issue_code(kind: TypeSystemErrorKind) -> DefinitionIssueCode {
  match kind {
    TypeSystemErrorKind::IncompatibleTypes => DefinitionIssueCode::IncompatibleExpressionTypes,
    TypeSystemErrorKind::InvalidType => DefinitionIssueCode::InvalidExpressionType,
    TypeSystemErrorKind::InvalidFunctionArity => DefinitionIssueCode::InvalidFunctionArity,
    TypeSystemErrorKind::UnresolvedType => DefinitionIssueCode::UnresolvedExpressionType,
  }
}

fn infer_source_nullability(definition: &GraphDefinition) -> HashMap<&str, bool> {
  let source_keys: HashSet<_> = definition
    .sources
    .iter()
    .map(|source| source.key.as_str())
    .collect();
  if !source_keys.contains(definition.root.as_str()) {
    return HashMap::new();
  }

  let mut nullability = HashMap::from([(definition.root.as_str(), false)]);
  let mut queue = VecDeque::from([definition.root.as_str()]);

  while let Some(source) = queue.pop_front() {
    let parent_nullable = nullability.get(source).copied().unwrap_or(false);
    for relation in definition
      .relations
      .iter()
      .filter(|relation| relation.from == source && source_keys.contains(relation.to.as_str()))
    {
      let target = relation.to.as_str();
      let target_nullable = parent_nullable || !relation.required;
      let previous = nullability.insert(
        target,
        nullability.get(target).copied().unwrap_or(false) || target_nullable,
      );
      if previous.is_none() || previous == Some(false) && target_nullable {
        queue.push_back(target);
      }
    }
  }

  nullability
}
