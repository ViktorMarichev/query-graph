use std::collections::HashSet;

use crate::{
  analysis::{DefinitionIndex, GraphTopology},
  scalar::is_decimal_text,
  Expression, GraphDefinition, LiteralValue, ParameterShape,
};

use super::{DefinitionIssue, DefinitionIssueCode};

struct ExpressionScope<'a> {
  allowed_sources: AllowedSources<'a>,
  issue_code: DefinitionIssueCode,
}

enum AllowedSources<'a> {
  Explicit(&'a HashSet<String>),
  PathTo(usize),
}

#[derive(Clone, Copy)]
enum ExistsPolicy {
  Deny,
  AllowImplicitCorrelation,
  RequireExplicitCorrelation,
}

pub(super) struct ExpressionContext<'a> {
  definition: &'a GraphDefinition,
  index: &'a DefinitionIndex,
  topology: &'a GraphTopology,
  scope: Option<ExpressionScope<'a>>,
  exists_policy: ExistsPolicy,
}

impl<'a> ExpressionContext<'a> {
  pub(super) fn unrestricted(
    definition: &'a GraphDefinition,
    index: &'a DefinitionIndex,
    topology: &'a GraphTopology,
  ) -> Self {
    Self {
      definition,
      index,
      topology,
      scope: None,
      exists_policy: ExistsPolicy::Deny,
    }
  }

  pub(super) fn constraint(
    definition: &'a GraphDefinition,
    index: &'a DefinitionIndex,
    topology: &'a GraphTopology,
  ) -> Self {
    Self {
      definition,
      index,
      topology,
      scope: None,
      exists_policy: ExistsPolicy::AllowImplicitCorrelation,
    }
  }

  pub(super) fn scoped(
    definition: &'a GraphDefinition,
    index: &'a DefinitionIndex,
    topology: &'a GraphTopology,
    allowed_sources: &'a HashSet<String>,
    scope_issue_code: DefinitionIssueCode,
  ) -> Self {
    Self {
      definition,
      index,
      topology,
      scope: Some(ExpressionScope {
        allowed_sources: AllowedSources::Explicit(allowed_sources),
        issue_code: scope_issue_code,
      }),
      exists_policy: ExistsPolicy::Deny,
    }
  }

  pub(super) fn relation_predicate(
    definition: &'a GraphDefinition,
    index: &'a DefinitionIndex,
    topology: &'a GraphTopology,
    allowed_sources: &'a HashSet<String>,
  ) -> Self {
    Self {
      definition,
      index,
      topology,
      scope: Some(ExpressionScope {
        allowed_sources: AllowedSources::Explicit(allowed_sources),
        issue_code: DefinitionIssueCode::RelationExpressionScope,
      }),
      exists_policy: ExistsPolicy::RequireExplicitCorrelation,
    }
  }

  fn within_exists(&self, source: usize) -> Self {
    Self {
      definition: self.definition,
      index: self.index,
      topology: self.topology,
      scope: Some(ExpressionScope {
        allowed_sources: AllowedSources::PathTo(source),
        issue_code: DefinitionIssueCode::ExistsExpressionScope,
      }),
      exists_policy: self.exists_policy,
    }
  }

  fn source_is_allowed(&self, source: &str) -> bool {
    let Some(scope) = &self.scope else {
      return true;
    };

    match scope.allowed_sources {
      AllowedSources::Explicit(sources) => sources.contains(source),
      AllowedSources::PathTo(target) => self.index.source(source).is_some_and(|candidate| {
        self
          .topology
          .relation_path_between(candidate, target)
          .is_some()
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
  match context.exists_policy {
    ExistsPolicy::Deny => {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::InvalidExistsContext,
        location,
        "exists expressions are not allowed in this expression context",
      ));
    }
    ExistsPolicy::RequireExplicitCorrelation if from.is_none() => {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::InvalidExistsSource,
        format!("{location}.from"),
        "exists in a relation predicate must declare an explicit correlation source",
      ));
    }
    ExistsPolicy::AllowImplicitCorrelation | ExistsPolicy::RequireExplicitCorrelation => {}
  }

  let Some(source_index) = context.index.source(source) else {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownExistsSource,
      format!("{location}.source"),
      format!("exists source {source:?} is not defined"),
    ));
    if let Some(predicate) = predicate {
      validate(predicate, &format!("{location}.predicate"), context, issues);
    }
    return;
  };

  let Some(source_path) = context.topology.relation_path(source_index) else {
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
    if let Some(from_index) = context.index.source(from) {
      if from == source
        || context
          .topology
          .relation_path_between(from_index, source_index)
          .is_none()
      {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::InvalidExistsSource,
          format!("{location}.from"),
          format!("exists source {source:?} is not a descendant of correlation source {from:?}"),
        ));
      } else if let Some(scope) = &context.scope {
        if !context.source_is_allowed(from) {
          issues.push(DefinitionIssue::new(
            scope.issue_code,
            format!("{location}.from"),
            format!("exists correlation source {from:?} is outside the current expression scope"),
          ));
        }
      }
    } else {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownExistsSource,
        format!("{location}.from"),
        format!("exists correlation source {from:?} is not defined"),
      ));
    }
  }

  if source_path.is_empty() {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidExistsSource,
      format!("{location}.source"),
      "exists source must be a descendant of the graph root",
    ));
  }

  if let Some(predicate) = predicate {
    let predicate_context = context.within_exists(source_index);
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
  let Some(parameter_index) = context.index.parameter(name) else {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownParameter,
      location,
      format!("parameter {name:?} is not defined"),
    ));
    return;
  };
  let actual_shape = context.definition.parameters[parameter_index].shape;

  if actual_shape != expected_shape {
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
  let Some(source_index) = context.index.source(source) else {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownFieldSource,
      location,
      format!("source {source:?} is not defined"),
    ));
    return;
  };

  if context.index.field(source_index, field).is_none() {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownField,
      location,
      format!("field {source:?}.{field:?} is not defined"),
    ));
  }

  if let Some(scope) = &context.scope {
    if !context.source_is_allowed(source) {
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
