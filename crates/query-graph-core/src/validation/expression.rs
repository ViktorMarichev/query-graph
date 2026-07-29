use std::collections::{HashMap, HashSet};

use crate::{scalar::is_decimal_text, Expression, LiteralValue, ParameterShape};

use super::{DefinitionIssue, DefinitionIssueCode};

struct ExpressionScope<'a> {
  allowed_sources: &'a HashSet<String>,
  issue_code: DefinitionIssueCode,
}

pub(super) struct ExpressionContext<'a> {
  sources: &'a HashMap<String, HashSet<String>>,
  parameters: &'a HashMap<String, ParameterShape>,
  source_scopes: &'a HashMap<String, HashSet<String>>,
  scope: Option<ExpressionScope<'a>>,
  allow_exists: bool,
}

impl<'a> ExpressionContext<'a> {
  pub(super) fn unrestricted(
    sources: &'a HashMap<String, HashSet<String>>,
    parameters: &'a HashMap<String, ParameterShape>,
    source_scopes: &'a HashMap<String, HashSet<String>>,
  ) -> Self {
    Self {
      sources,
      parameters,
      source_scopes,
      scope: None,
      allow_exists: false,
    }
  }

  pub(super) fn constraint(
    sources: &'a HashMap<String, HashSet<String>>,
    parameters: &'a HashMap<String, ParameterShape>,
    source_scopes: &'a HashMap<String, HashSet<String>>,
  ) -> Self {
    Self {
      sources,
      parameters,
      source_scopes,
      scope: None,
      allow_exists: true,
    }
  }

  pub(super) fn scoped(
    sources: &'a HashMap<String, HashSet<String>>,
    parameters: &'a HashMap<String, ParameterShape>,
    source_scopes: &'a HashMap<String, HashSet<String>>,
    allowed_sources: &'a HashSet<String>,
    scope_issue_code: DefinitionIssueCode,
  ) -> Self {
    Self {
      sources,
      parameters,
      source_scopes,
      scope: Some(ExpressionScope {
        allowed_sources,
        issue_code: scope_issue_code,
      }),
      allow_exists: false,
    }
  }

  fn within_exists(&self, allowed_sources: &'a HashSet<String>) -> Self {
    Self {
      sources: self.sources,
      parameters: self.parameters,
      source_scopes: self.source_scopes,
      scope: Some(ExpressionScope {
        allowed_sources,
        issue_code: DefinitionIssueCode::ExistsExpressionScope,
      }),
      allow_exists: self.allow_exists,
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
      validate_parameter(name, ParameterShape::Scalar, location, context, issues);
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
    Expression::InParameter {
      expression,
      parameter,
    } => {
      validate_child(expression, location, "expression", context, issues);
      validate_parameter(
        parameter,
        ParameterShape::List,
        &format!("{location}.parameter"),
        context,
        issues,
      );
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
    Expression::Aggregate { expression, .. } => {
      if let Some(expression) = expression {
        validate_child(expression, location, "expression", context, issues);
      }
    }
    Expression::Exists {
      source,
      from,
      predicate,
    } => {
      validate_exists(
        source,
        from.as_deref(),
        predicate.as_deref(),
        location,
        context,
        issues,
      );
    }
  }
}

fn validate_exists(
  source: &str,
  from: Option<&str>,
  predicate: Option<&Expression>,
  location: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  if !context.allow_exists {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidExistsContext,
      location,
      "exists expressions are allowed only in graph constraints",
    ));
  }

  if !context.sources.contains_key(source) {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownExistsSource,
      format!("{location}.source"),
      format!("exists source {source:?} is not defined"),
    ));
    if let Some(predicate) = predicate {
      validate(predicate, &format!("{location}.predicate"), context, issues);
    }
    return;
  }

  let Some(allowed_sources) = context.source_scopes.get(source) else {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidExistsSource,
      format!("{location}.source"),
      format!("exists source {source:?} is not reachable from the graph root"),
    ));
    if let Some(predicate) = predicate {
      validate(predicate, &format!("{location}.predicate"), context, issues);
    }
    return;
  };
  if let Some(from) = from {
    if !context.sources.contains_key(from) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownExistsSource,
        format!("{location}.from"),
        format!("exists correlation source {from:?} is not defined"),
      ));
    } else if from == source || !allowed_sources.contains(from) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::InvalidExistsSource,
        format!("{location}.from"),
        format!("exists source {source:?} is not a descendant of correlation source {from:?}"),
      ));
    } else if let Some(scope) = &context.scope {
      if !scope.allowed_sources.contains(from) {
        issues.push(DefinitionIssue::new(
          scope.issue_code,
          format!("{location}.from"),
          format!("exists correlation source {from:?} is outside the current expression scope"),
        ));
      }
    }
  }

  if allowed_sources.len() == 1 {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidExistsSource,
      format!("{location}.source"),
      "exists source must be a descendant of the graph root",
    ));
  }

  if let Some(predicate) = predicate {
    let predicate_context = context.within_exists(allowed_sources);
    validate(
      predicate,
      &format!("{location}.predicate"),
      &predicate_context,
      issues,
    );
  }
}

fn validate_parameter(
  name: &str,
  expected_shape: ParameterShape,
  location: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  let Some(actual_shape) = context.parameters.get(name) else {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownParameter,
      location,
      format!("parameter {name:?} is not defined"),
    ));
    return;
  };

  if *actual_shape != expected_shape {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidParameterShape,
      location,
      format!(
        "parameter {name:?} has shape {}, expected {}",
        actual_shape.as_str(),
        expected_shape.as_str()
      ),
    ));
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
