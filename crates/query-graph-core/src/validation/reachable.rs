use super::expression::{self, ExpressionContext};
use crate::{
  ConstraintDefinition, DefinitionIssue, DefinitionIssueCode, Expression, ReachableExpression,
  ScalarType,
};

pub(super) fn validate(
  traversal: &ReachableExpression,
  location: &str,
  context: &ExpressionContext<'_>,
  issues: &mut Vec<DefinitionIssue>,
) {
  let before = issues.len();
  let mut issue = |suffix: &str, message: &str| {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::InvalidReachableDefinition,
      format!("{location}.{suffix}"),
      message,
    ))
  };
  if traversal.scope.trim().is_empty() {
    issue("scope", "reachable scope must not be empty");
  }
  if !traversal.input.projection.fields.is_empty()
    || !traversal.input.projection.objects.is_empty()
    || !traversal.input.orderings.is_empty()
  {
    issue(
      "input",
      "reachable input cannot define projection or ordering",
    );
  }
  for source in &traversal.input.sources {
    if context
      .definition
      .sources
      .iter()
      .any(|outer| outer.key == source.key)
    {
      issue(
        "input.sources",
        "reachable input source keys must differ from outer source keys",
      );
    }
  }
  let mut parameter_names = std::collections::HashSet::new();
  for parameter in &traversal.input.parameters {
    if !parameter_names.insert(&parameter.name) {
      issue(
        "input.parameters",
        "reachable module parameters must not contain duplicate names",
      );
    }
    if !context.definition.parameters.contains(parameter) {
      issue(
        "input.parameters",
        "reachable module parameters must be declared identically on the outer graph",
      );
    }
  }
  let root = traversal
    .input
    .sources
    .iter()
    .find(|source| source.key == traversal.input.root);
  if !root
    .and_then(|source| {
      source
        .fields
        .iter()
        .find(|field| field.name == traversal.key)
    })
    .is_some_and(|field| {
      !field.nullable
        && matches!(
          field.scalar_type,
          ScalarType::Int32 | ScalarType::Int64 | ScalarType::String
        )
    })
  {
    issue("key", "reachable key must be a non-null integer or string field of its source; it must uniquely identify rows");
  }
  let mut input = traversal.input.clone();
  input.parameters = context.definition.parameters.clone();
  let step = step_predicate(&traversal.step, traversal);
  if step.is_none() {
    issue("step", "reachable step must be equalities between current and next fields, optionally combined with and");
  }
  for constraint in &input.constraints {
    if !valid_filter(&constraint.predicate, &input.root) {
      issue("input.constraints", "reachable input filters may reference only its root and local exists; nested traversals and aggregates are not supported");
    }
  }
  for relation in &input.relations {
    if contains_traversal(&relation.on) {
      issue(
        "input.relations",
        "nested traversal expressions are not supported",
      );
    }
  }
  if !valid_filter(&traversal.start, &input.root) {
    issue(
      "start",
      "reachable start must reference only its input root and local exists",
    );
  }
  if traversal.predicate.contains_aggregate() {
    issue("match", "reachable match cannot contain aggregates");
  }
  if issues.len() != before {
    return;
  }
  let start_index = input.constraints.len();
  input
    .constraints
    .push(ConstraintDefinition::always(traversal.start.clone()));
  input.constraints.push(ConstraintDefinition::always(
    step.expect("step checked above"),
  ));
  if let Err(errors) = input.compile() {
    for mut error in errors.into_vec() {
      let start = format!("constraints[{start_index}].predicate");
      let step = format!("constraints[{}].predicate", start_index + 1);
      error.location = if let Some(suffix) = error.location.strip_prefix(&start) {
        format!("{location}.start{suffix}")
      } else if let Some(suffix) = error.location.strip_prefix(&step) {
        format!("{location}.step{suffix}")
      } else {
        format!("{location}.input.{}", error.location)
      };
      issues.push(error);
    }
  }
  expression::validate(
    &traversal.predicate,
    &format!("{location}.match"),
    &context.traversal_match(traversal),
    issues,
  );
}

fn contains_traversal(expression: &Expression) -> bool {
  let mut found = false;
  expression.walk(&mut |node| {
    if matches!(
      node,
      Expression::Reachable(_) | Expression::TraversalField { .. }
    ) {
      found = true;
      return false;
    }
    true
  });
  found
}

fn valid_filter(expression: &Expression, root: &str) -> bool {
  if contains_traversal(expression) || expression.contains_aggregate() {
    return false;
  }
  let mut valid = true;
  expression.walk(&mut |node| {
    match node {
      Expression::Field { source, .. } if source != root => valid = false,
      Expression::Exists { from, .. } => {
        if from.as_deref().is_some_and(|from| from != root) {
          valid = false;
        }
        return false;
      }
      _ => {}
    }
    true
  });
  valid
}

fn step_predicate(expression: &Expression, traversal: &ReachableExpression) -> Option<Expression> {
  match expression {
    Expression::And { expressions } if !expressions.is_empty() => Some(Expression::and(
      expressions
        .iter()
        .map(|node| step_predicate(node, traversal))
        .collect::<Option<Vec<_>>>()?,
    )),
    Expression::Eq { left, right } => {
      let Expression::TraversalField {
        scope: left_scope,
        side: left_side,
        field: left_field,
      } = left.as_ref()
      else {
        return None;
      };
      let Expression::TraversalField {
        scope: right_scope,
        side: right_side,
        field: right_field,
      } = right.as_ref()
      else {
        return None;
      };
      if left_scope != &traversal.scope
        || right_scope != &traversal.scope
        || left_side == right_side
      {
        return None;
      }
      Some(Expression::eq(
        Expression::field(&traversal.input.root, left_field),
        Expression::field(&traversal.input.root, right_field),
      ))
    }
    _ => None,
  }
}
