use std::collections::{HashMap, HashSet};

use crate::{scalar::is_decimal_text, Expression, LiteralValue};

use super::{DefinitionIssue, DefinitionIssueCode};

struct ExpressionScope<'a> {
  allowed_sources: &'a HashSet<&'a str>,
  issue_code: DefinitionIssueCode,
}

pub(super) struct ExpressionContext<'a> {
  sources: &'a HashMap<String, HashSet<String>>,
  parameters: &'a HashSet<String>,
  scope: Option<ExpressionScope<'a>>,
}

impl<'a> ExpressionContext<'a> {
  pub(super) fn unrestricted(
    sources: &'a HashMap<String, HashSet<String>>,
    parameters: &'a HashSet<String>,
  ) -> Self {
    Self {
      sources,
      parameters,
      scope: None,
    }
  }

  pub(super) fn scoped(
    sources: &'a HashMap<String, HashSet<String>>,
    parameters: &'a HashSet<String>,
    allowed_sources: &'a HashSet<&'a str>,
    scope_issue_code: DefinitionIssueCode,
  ) -> Self {
    Self {
      sources,
      parameters,
      scope: Some(ExpressionScope {
        allowed_sources,
        issue_code: scope_issue_code,
      }),
    }
  }
}

pub(super) fn validate(
  expression: &Expression,
  location: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  match expression {
    Expression::Field { source, field } => {
      validate_field(source, field, location, context, issues);
    }
    Expression::Parameter { name } => {
      if !context.parameters.contains(name) {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownParameter,
          location,
          format!("parameter {name:?} is not defined"),
        ));
      }
    }
    Expression::Literal {
      value: LiteralValue::Decimal(value),
    } if !is_decimal_text(value) => {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::InvalidLiteral,
        format!("{location}.value"),
        format!("decimal literal {value:?} is invalid"),
      ));
    }
    Expression::Literal { .. } => {}
    Expression::Eq { left, right }
    | Expression::NotEq { left, right }
    | Expression::LessThan { left, right }
    | Expression::LessThanOrEqual { left, right }
    | Expression::GreaterThan { left, right }
    | Expression::GreaterThanOrEqual { left, right } => {
      validate_child(left, location, "left", context, issues);
      validate_child(right, location, "right", context, issues);
    }
    Expression::Like {
      expression,
      pattern,
    } => {
      validate_child(expression, location, "expression", context, issues);
      validate_child(pattern, location, "pattern", context, issues);
    }
    Expression::In { expression, values } => {
      validate_child(expression, location, "expression", context, issues);
      validate_non_empty(
        values,
        &format!("{location}.values"),
        "in expression must contain at least one value",
        issues,
      );
      validate_children(values, &format!("{location}.values"), context, issues);
    }
    Expression::And { expressions } | Expression::Or { expressions } => {
      validate_non_empty(
        expressions,
        &format!("{location}.expressions"),
        "logical expression must contain at least one item",
        issues,
      );
      validate_children(
        expressions,
        &format!("{location}.expressions"),
        context,
        issues,
      );
    }
    Expression::Not { expression }
    | Expression::IsNull { expression }
    | Expression::IsNotNull { expression } => {
      validate_child(expression, location, "expression", context, issues);
    }
    Expression::Function { arguments, .. } => {
      validate_children(arguments, &format!("{location}.arguments"), context, issues);
    }
  }
}

fn validate_field(
  source: &str,
  field: &str,
  location: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  let Some(fields) = context.sources.get(source) else {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownFieldSource,
      location,
      format!("source {source:?} is not defined"),
    ));
    return;
  };

  if !fields.contains(field) {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownField,
      location,
      format!("field {source:?}.{field:?} is not defined"),
    ));
  }

  if let Some(scope) = &context.scope {
    if !scope.allowed_sources.contains(source) {
      issues.push(DefinitionIssue::new(
        scope.issue_code,
        location,
        format!("source {source:?} is outside the expression scope"),
      ));
    }
  }
}

fn validate_child(
  expression: &Expression,
  location: &str,
  role: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  validate(expression, &format!("{location}.{role}"), context, issues);
}

fn validate_children(
  expressions: &[Expression],
  location: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  for (index, expression) in expressions.iter().enumerate() {
    validate(expression, &format!("{location}[{index}]"), context, issues);
  }
}

fn validate_non_empty(
  expressions: &[Expression],
  location: &str,
  message: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
  if expressions.is_empty() {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::EmptyExpressionGroup,
      location,
      message,
    ));
  }
}
