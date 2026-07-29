use crate::{Expression, GraphDefinition, ProjectionFieldDefinition, ProjectionFieldRole};

use super::{DefinitionIssue, DefinitionIssueCode};

pub(super) fn validate(definition: &GraphDefinition, issues: &mut Vec<DefinitionIssue>) {
  validate_relation_expressions(definition, issues);

  if !definition.is_summary() {
    validate_record_graph(definition, issues);
    return;
  }

  let dimensions: Vec<_> = definition
    .projection
    .fields
    .iter()
    .filter(|field| field.role == ProjectionFieldRole::Dimension)
    .collect();

  for (index, field) in definition.projection.fields.iter().enumerate() {
    let location = format!("projection.fields[{index}]");
    match field.role {
      ProjectionFieldRole::Value => issues.push(DefinitionIssue::new(
        DefinitionIssueCode::MixedProjectionRoles,
        format!("{location}.role"),
        "summary graph projection cannot contain regular value fields",
      )),
      ProjectionFieldRole::Dimension => {
        if field.expression.contains_aggregate() {
          issues.push(DefinitionIssue::new(
            DefinitionIssueCode::InvalidDimensionExpression,
            format!("{location}.expression"),
            "dimension expression cannot contain an aggregate",
          ));
        }
      }
      ProjectionFieldRole::Measure => {
        if !field.expression.contains_aggregate() {
          issues.push(DefinitionIssue::new(
            DefinitionIssueCode::InvalidMeasureExpression,
            format!("{location}.expression"),
            "measure expression must contain an aggregate",
          ));
        } else {
          validate_grouped_expression(
            &field.expression,
            &format!("{location}.expression"),
            &dimensions,
            issues,
          );
        }
      }
    }

    validate_nested_aggregates(
      &field.expression,
      &format!("{location}.expression"),
      false,
      issues,
    );
  }

  for (index, constraint) in definition.constraints.iter().enumerate() {
    let location = format!("constraints[{index}].predicate");
    validate_nested_aggregates(&constraint.predicate, &location, false, issues);
    if constraint.predicate.contains_aggregate() {
      validate_grouped_expression(&constraint.predicate, &location, &dimensions, issues);
    }
  }

  for (ordering_index, ordering) in definition.orderings.iter().enumerate() {
    for (order_index, order) in ordering.order_by.iter().enumerate() {
      let location = format!("orderings[{ordering_index}].orderBy[{order_index}].expression");
      validate_nested_aggregates(&order.expression, &location, false, issues);
      validate_grouped_expression(&order.expression, &location, &dimensions, issues);
    }
  }
}

fn validate_record_graph(definition: &GraphDefinition, issues: &mut Vec<DefinitionIssue>) {
  for (index, field) in definition.projection.fields.iter().enumerate() {
    reject_aggregate(
      &field.expression,
      &format!("projection.fields[{index}].expression"),
      "aggregate expressions require a summary graph",
      issues,
    );
  }

  for (index, constraint) in definition.constraints.iter().enumerate() {
    reject_aggregate(
      &constraint.predicate,
      &format!("constraints[{index}].predicate"),
      "aggregate constraints require a summary graph",
      issues,
    );
  }

  for (ordering_index, ordering) in definition.orderings.iter().enumerate() {
    for (order_index, order) in ordering.order_by.iter().enumerate() {
      reject_aggregate(
        &order.expression,
        &format!("orderings[{ordering_index}].orderBy[{order_index}].expression"),
        "aggregate ordering requires a summary graph",
        issues,
      );
    }
  }
}

fn validate_relation_expressions(definition: &GraphDefinition, issues: &mut Vec<DefinitionIssue>) {
  for (index, relation) in definition.relations.iter().enumerate() {
    reject_aggregate(
      &relation.on,
      &format!("relations[{index}].on"),
      "relation predicates cannot contain aggregates",
      issues,
    );

    if let Some(selection) = &relation.selection {
      for (order_index, order) in selection.order_by().iter().enumerate() {
        reject_aggregate(
          &order.expression,
          &format!("relations[{index}].selection.orderBy[{order_index}].expression"),
          "relation selection ordering cannot contain aggregates",
          issues,
        );
      }
    }
  }
}

fn reject_aggregate(
  expression: &Expression,
  location: &str,
  message: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
  if expression.contains_aggregate() {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidAggregateContext,
      location,
      message,
    ));
  }
}

fn validate_nested_aggregates(
  expression: &Expression,
  location: &str,
  inside_aggregate: bool,
  issues: &mut Vec<DefinitionIssue>,
) {
  if inside_aggregate && is_predicate(expression) {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidAggregateContext,
      location,
      "predicate expressions cannot be aggregate arguments",
    ));
  }

  let inside_children = if matches!(expression, Expression::Aggregate { .. }) {
    if inside_aggregate {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::NestedAggregate,
        location,
        "aggregate expressions cannot be nested",
      ));
    }
    true
  } else {
    inside_aggregate
  };

  visit_children(expression, location, &mut |child, child_location| {
    validate_nested_aggregates(child, &child_location, inside_children, issues);
  });
}

fn is_predicate(expression: &Expression) -> bool {
  matches!(
    expression,
    Expression::Eq { .. }
      | Expression::NotEq { .. }
      | Expression::LessThan { .. }
      | Expression::LessThanOrEqual { .. }
      | Expression::GreaterThan { .. }
      | Expression::GreaterThanOrEqual { .. }
      | Expression::Like { .. }
      | Expression::In { .. }
      | Expression::InParameter { .. }
      | Expression::And { .. }
      | Expression::Or { .. }
      | Expression::Not { .. }
      | Expression::IsNull { .. }
      | Expression::IsNotNull { .. }
      | Expression::Exists { .. }
  )
}

fn validate_grouped_expression(
  expression: &Expression,
  location: &str,
  dimensions: &[&ProjectionFieldDefinition],
  issues: &mut Vec<DefinitionIssue>,
) {
  if dimensions
    .iter()
    .any(|dimension| dimension.expression == *expression)
  {
    return;
  }

  match expression {
    Expression::Field { source, field } => {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UngroupedExpression,
        location,
        format!("field {source:?}.{field:?} is not a declared dimension"),
      ));
      return;
    }
    Expression::Exists { .. } => {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UngroupedExpression,
        location,
        "exists cannot be combined with aggregates in one grouped expression",
      ));
      return;
    }
    Expression::Parameter { .. } | Expression::Literal { .. } | Expression::Aggregate { .. } => {
      return;
    }
    _ => {}
  }

  visit_children(expression, location, &mut |child, child_location| {
    validate_grouped_expression(child, &child_location, dimensions, issues);
  });
}

fn visit_children(
  expression: &Expression,
  location: &str,
  visitor: &mut impl FnMut(&Expression, String),
) {
  match expression {
    Expression::Eq { left, right }
    | Expression::NotEq { left, right }
    | Expression::LessThan { left, right }
    | Expression::LessThanOrEqual { left, right }
    | Expression::GreaterThan { left, right }
    | Expression::GreaterThanOrEqual { left, right } => {
      visitor(left, format!("{location}.left"));
      visitor(right, format!("{location}.right"));
    }
    Expression::Like {
      expression,
      pattern,
    } => {
      visitor(expression, format!("{location}.expression"));
      visitor(pattern, format!("{location}.pattern"));
    }
    Expression::In { expression, values } => {
      visitor(expression, format!("{location}.expression"));
      for (index, value) in values.iter().enumerate() {
        visitor(value, format!("{location}.values[{index}]"));
      }
    }
    Expression::InParameter { expression, .. }
    | Expression::Not { expression }
    | Expression::IsNull { expression }
    | Expression::IsNotNull { expression } => {
      visitor(expression, format!("{location}.expression"));
    }
    Expression::And { expressions } | Expression::Or { expressions } => {
      for (index, expression) in expressions.iter().enumerate() {
        visitor(expression, format!("{location}.expressions[{index}]"));
      }
    }
    Expression::Exists { predicate, .. } => {
      if let Some(predicate) = predicate {
        visitor(predicate, format!("{location}.predicate"));
      }
    }
    Expression::Function { arguments, .. } => {
      for (index, argument) in arguments.iter().enumerate() {
        visitor(argument, format!("{location}.arguments[{index}]"));
      }
    }
    Expression::Aggregate { expression, .. } => {
      if let Some(expression) = expression {
        visitor(expression, format!("{location}.expression"));
      }
    }
    Expression::Field { .. } | Expression::Parameter { .. } | Expression::Literal { .. } => {}
  }
}
