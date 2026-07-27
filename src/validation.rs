use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ConstraintCondition, Expression, GraphDefinition, GRAPH_DEFINITION_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionIssue {
  pub code: DefinitionIssueCode,
  pub location: String,
  pub message: String,
}

impl DefinitionIssue {
  fn new(
    code: DefinitionIssueCode,
    location: impl Into<String>,
    message: impl Into<String>,
  ) -> Self {
    Self {
      code,
      location: location.into(),
      message: message.into(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DefinitionIssueCode {
  UnsupportedVersion,
  EmptyName,
  EmptySourceKey,
  DuplicateSource,
  UnknownRoot,
  EmptyFieldName,
  DuplicateField,
  EmptyParameterName,
  DuplicateParameter,
  EmptyRelationName,
  DuplicateRelation,
  UnknownRelationSource,
  UnknownRelationTarget,
  RelationExpressionScope,
  EmptyConstraintName,
  DuplicateConstraint,
  UnknownFieldSource,
  UnknownField,
  UnknownParameter,
  EmptyExpressionGroup,
  EmptyFunctionName,
  EmptyProjectionPath,
  EmptyProjectionPathSegment,
  InvalidProjectionPathSegment,
  DuplicateProjectionPath,
  ConflictingProjectionPath,
  HiddenProjectionField,
  NonSelectableDefaultProjection,
  UnknownProjectionRelation,
  InvalidProjectionRelationPath,
  ProjectionExpressionScope,
  RootHasIncomingRelation,
  AmbiguousSourcePath,
  UnreachableSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DefinitionIssues(Vec<DefinitionIssue>);

impl DefinitionIssues {
  pub fn as_slice(&self) -> &[DefinitionIssue] {
    &self.0
  }

  pub fn into_vec(self) -> Vec<DefinitionIssue> {
    self.0
  }
}

impl fmt::Display for DefinitionIssues {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "query graph definition contains {} issue(s)",
      self.0.len()
    )?;

    for issue in &self.0 {
      write!(
        formatter,
        "\n- {:?} at {}: {}",
        issue.code, issue.location, issue.message
      )?;
    }

    Ok(())
  }
}

impl Error for DefinitionIssues {}

pub(crate) fn validate(definition: &GraphDefinition) -> Result<(), DefinitionIssues> {
  let mut issues = Vec::new();

  if definition.schema_version != GRAPH_DEFINITION_VERSION {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnsupportedVersion,
      "schemaVersion",
      format!(
        "expected version {}, received {}",
        GRAPH_DEFINITION_VERSION, definition.schema_version
      ),
    ));
  }

  if definition.name.trim().is_empty() {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::EmptyName,
      "name",
      "graph name must not be empty",
    ));
  }

  let mut sources = HashMap::<String, HashSet<String>>::new();
  for (source_index, source) in definition.sources.iter().enumerate() {
    let source_location = format!("sources[{source_index}]");
    if source.key.trim().is_empty() {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptySourceKey,
        format!("{source_location}.key"),
        "source key must not be empty",
      ));
      continue;
    }

    if sources.contains_key(&source.key) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::DuplicateSource,
        format!("{source_location}.key"),
        format!("source {:?} is defined more than once", source.key),
      ));
      continue;
    }

    let mut fields = HashSet::new();
    for (field_index, field) in source.fields.iter().enumerate() {
      let field_location = format!("{source_location}.fields[{field_index}].name");
      if field.name.trim().is_empty() {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyFieldName,
          field_location,
          "field name must not be empty",
        ));
        continue;
      }

      if !fields.insert(field.name.clone()) {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateField,
          field_location,
          format!(
            "field {:?} is defined more than once in source {:?}",
            field.name, source.key
          ),
        ));
      }
    }
    sources.insert(source.key.clone(), fields);
  }

  if !sources.contains_key(&definition.root) {
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::UnknownRoot,
      "root",
      format!("root source {:?} is not defined", definition.root),
    ));
  }

  let mut parameters = HashSet::new();
  for (parameter_index, parameter) in definition.parameters.iter().enumerate() {
    let location = format!("parameters[{parameter_index}].name");
    if parameter.name.trim().is_empty() {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyParameterName,
        location,
        "parameter name must not be empty",
      ));
      continue;
    }

    if !parameters.insert(parameter.name.clone()) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::DuplicateParameter,
        location,
        format!("parameter {:?} is defined more than once", parameter.name),
      ));
    }
  }

  let mut relations = HashMap::new();
  for (relation_index, relation) in definition.relations.iter().enumerate() {
    let location = format!("relations[{relation_index}]");
    if relation.name.trim().is_empty() {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyRelationName,
        format!("{location}.name"),
        "relation name must not be empty",
      ));
    } else if relations
      .insert(relation.name.clone(), relation_index)
      .is_some()
    {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::DuplicateRelation,
        format!("{location}.name"),
        format!("relation {:?} is defined more than once", relation.name),
      ));
    }

    if !sources.contains_key(&relation.from) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownRelationSource,
        format!("{location}.from"),
        format!("relation source {:?} is not defined", relation.from),
      ));
    }

    if !sources.contains_key(&relation.to) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownRelationTarget,
        format!("{location}.to"),
        format!("relation target {:?} is not defined", relation.to),
      ));
    }

    let allowed_sources = HashSet::from([relation.from.as_str(), relation.to.as_str()]);
    validate_expression(
      &relation.on,
      &format!("{location}.on"),
      &sources,
      &parameters,
      Some(&allowed_sources),
      DefinitionIssueCode::RelationExpressionScope,
      &mut issues,
    );
  }

  let mut constraint_names = HashSet::new();
  for (constraint_index, constraint) in definition.constraints.iter().enumerate() {
    let location = format!("constraints[{constraint_index}]");
    if constraint.name.trim().is_empty() {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyConstraintName,
        format!("{location}.name"),
        "constraint name must not be empty",
      ));
    } else if !constraint_names.insert(constraint.name.clone()) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::DuplicateConstraint,
        format!("{location}.name"),
        format!("constraint {:?} is defined more than once", constraint.name),
      ));
    }

    if let ConstraintCondition::ParameterPresent { parameter } = &constraint.when {
      if !parameters.contains(parameter) {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownParameter,
          format!("{location}.when.parameter"),
          format!("parameter {:?} is not defined", parameter),
        ));
      }
    }

    validate_expression(
      &constraint.predicate,
      &format!("{location}.predicate"),
      &sources,
      &parameters,
      None,
      DefinitionIssueCode::RelationExpressionScope,
      &mut issues,
    );
  }

  let mut projection_paths: HashSet<Vec<String>> = HashSet::new();
  for (field_index, field) in definition.projection.fields.iter().enumerate() {
    let location = format!("projection.fields[{field_index}]");
    if field.path.is_empty() {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyProjectionPath,
        format!("{location}.path"),
        "projection path must contain at least one segment",
      ));
    }

    for (segment_index, segment) in field.path.iter().enumerate() {
      let segment_location = format!("{location}.path[{segment_index}]");
      if segment.trim().is_empty() {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyProjectionPathSegment,
          segment_location,
          "projection path segment must not be empty",
        ));
      } else if segment.contains('.') {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::InvalidProjectionPathSegment,
          segment_location,
          "projection path segment must not contain '.'",
        ));
      }
    }

    let conflicting_path = projection_paths
      .iter()
      .find(|existing| projection_paths_conflict(existing, &field.path))
      .cloned();

    if !projection_paths.insert(field.path.clone()) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::DuplicateProjectionPath,
        format!("{location}.path"),
        format!("projection path {:?} is defined more than once", field.path),
      ));
    } else if let Some(conflicting_path) = conflicting_path {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::ConflictingProjectionPath,
        format!("{location}.path"),
        format!(
          "projection path {:?} conflicts with {:?}",
          field.path, conflicting_path
        ),
      ));
    }

    if field.selected_by_default && !field.selectable {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::NonSelectableDefaultProjection,
        format!("{location}.selectedByDefault"),
        "a non-selectable projection field cannot be selected by default",
      ));
    }

    let visited_sources =
      validate_projection_relation_path(definition, &relations, field, &location, &mut issues);

    let allowed_sources: HashSet<&str> = visited_sources.iter().map(String::as_str).collect();
    validate_expression(
      &field.expression,
      &format!("{location}.expression"),
      &sources,
      &parameters,
      Some(&allowed_sources),
      DefinitionIssueCode::ProjectionExpressionScope,
      &mut issues,
    );

    validate_projection_visibility(definition, field, &location, &mut issues);
  }

  for (order_index, order) in definition.default_order_by.iter().enumerate() {
    validate_expression(
      &order.expression,
      &format!("defaultOrderBy[{order_index}].expression"),
      &sources,
      &parameters,
      None,
      DefinitionIssueCode::RelationExpressionScope,
      &mut issues,
    );
  }

  validate_topology(definition, &sources, &mut issues);

  if issues.is_empty() {
    Ok(())
  } else {
    Err(DefinitionIssues(issues))
  }
}

fn projection_paths_conflict(left: &[String], right: &[String]) -> bool {
  if left.is_empty() || right.is_empty() || left.len() == right.len() {
    return false;
  }

  if left.len() < right.len() {
    right.starts_with(left)
  } else {
    left.starts_with(right)
  }
}

fn validate_projection_relation_path(
  definition: &GraphDefinition,
  relations: &HashMap<String, usize>,
  field: &crate::ProjectionFieldDefinition,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) -> HashSet<String> {
  let mut current_source = definition.root.clone();
  let mut visited_sources = HashSet::from([current_source.clone()]);

  for (path_index, relation_name) in field.relations.iter().enumerate() {
    let Some(relation_index) = relations.get(relation_name) else {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownProjectionRelation,
        format!("{location}.relations[{path_index}]"),
        format!("relation {:?} is not defined", relation_name),
      ));
      continue;
    };

    let relation = &definition.relations[*relation_index];
    if relation.from != current_source {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::InvalidProjectionRelationPath,
        format!("{location}.relations[{path_index}]"),
        format!(
          "relation {:?} starts at {:?}, but the current source is {:?}",
          relation.name, relation.from, current_source
        ),
      ));
      continue;
    }

    current_source = relation.to.clone();
    visited_sources.insert(current_source.clone());
  }

  visited_sources
}

fn validate_projection_visibility(
  definition: &GraphDefinition,
  projection: &crate::ProjectionFieldDefinition,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
  if !projection.selectable {
    return;
  }

  let mut referenced_fields = HashSet::new();
  projection.expression.for_each_field(&mut |source, field| {
    referenced_fields.insert((source, field));
  });

  for (source, field) in referenced_fields {
    let Some(field_definition) = definition
      .sources
      .iter()
      .find(|candidate| candidate.key == source)
      .and_then(|source| {
        source
          .fields
          .iter()
          .find(|candidate| candidate.name == field)
      })
    else {
      continue;
    };

    if !field_definition.selectable {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::HiddenProjectionField,
        format!("{location}.expression"),
        format!("field {source:?}.{field:?} is internal and cannot be exposed"),
      ));
    }
  }
}

fn validate_topology(
  definition: &GraphDefinition,
  sources: &HashMap<String, HashSet<String>>,
  issues: &mut Vec<DefinitionIssue>,
) {
  let mut incoming = HashMap::<&str, Vec<(usize, &str)>>::new();

  for (relation_index, relation) in definition.relations.iter().enumerate() {
    if !sources.contains_key(&relation.from) || !sources.contains_key(&relation.to) {
      continue;
    }

    if relation.to == definition.root {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::RootHasIncomingRelation,
        format!("relations[{relation_index}].to"),
        format!(
          "root source {:?} cannot have an incoming relation",
          definition.root
        ),
      ));
    }

    incoming
      .entry(&relation.to)
      .or_default()
      .push((relation_index, &relation.name));
  }

  for (source, relations) in incoming {
    if source != definition.root && relations.len() > 1 {
      let names: Vec<_> = relations.iter().map(|(_, name)| *name).collect();
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::AmbiguousSourcePath,
        format!("sources.{source}"),
        format!("source {source:?} has multiple incoming relations: {names:?}"),
      ));
    }
  }

  validate_reachability(definition, sources, issues);
}

fn validate_reachability(
  definition: &GraphDefinition,
  sources: &HashMap<String, HashSet<String>>,
  issues: &mut Vec<DefinitionIssue>,
) {
  if !sources.contains_key(&definition.root) {
    return;
  }

  let mut reachable = HashSet::from([definition.root.clone()]);
  let mut queue = VecDeque::from([definition.root.clone()]);

  while let Some(source) = queue.pop_front() {
    for relation in definition
      .relations
      .iter()
      .filter(|relation| relation.from == source)
    {
      if !sources.contains_key(&relation.to) {
        continue;
      }
      if reachable.insert(relation.to.clone()) {
        queue.push_back(relation.to.clone());
      }
    }
  }

  for (source_index, source) in definition.sources.iter().enumerate() {
    if !reachable.contains(&source.key) {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnreachableSource,
        format!("sources[{source_index}].key"),
        format!(
          "source {:?} is not reachable from root {:?}",
          source.key, definition.root
        ),
      ));
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression(
  expression: &Expression,
  location: &str,
  sources: &HashMap<String, HashSet<String>>,
  parameters: &HashSet<String>,
  allowed_sources: Option<&HashSet<&str>>,
  scope_issue_code: DefinitionIssueCode,
  issues: &mut Vec<DefinitionIssue>,
) {
  match expression {
    Expression::Field { source, field } => {
      let Some(fields) = sources.get(source) else {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownFieldSource,
          location,
          format!("source {:?} is not defined", source),
        ));
        return;
      };

      if !fields.contains(field) {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownField,
          location,
          format!("field {:?}.{:?} is not defined", source, field),
        ));
      }

      if let Some(allowed_sources) = allowed_sources {
        if !allowed_sources.contains(source.as_str()) {
          issues.push(DefinitionIssue::new(
            scope_issue_code,
            location,
            format!("source {:?} is outside the expression scope", source),
          ));
        }
      }
    }
    Expression::Parameter { name } => {
      if !parameters.contains(name) {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownParameter,
          location,
          format!("parameter {:?} is not defined", name),
        ));
      }
    }
    Expression::Literal { .. } => {}
    Expression::Eq { left, right }
    | Expression::NotEq { left, right }
    | Expression::LessThan { left, right }
    | Expression::LessThanOrEqual { left, right }
    | Expression::GreaterThan { left, right }
    | Expression::GreaterThanOrEqual { left, right } => {
      validate_expression(
        left,
        &format!("{location}.left"),
        sources,
        parameters,
        allowed_sources,
        scope_issue_code,
        issues,
      );
      validate_expression(
        right,
        &format!("{location}.right"),
        sources,
        parameters,
        allowed_sources,
        scope_issue_code,
        issues,
      );
    }
    Expression::Like {
      expression,
      pattern,
    } => {
      validate_expression(
        expression,
        &format!("{location}.expression"),
        sources,
        parameters,
        allowed_sources,
        scope_issue_code,
        issues,
      );
      validate_expression(
        pattern,
        &format!("{location}.pattern"),
        sources,
        parameters,
        allowed_sources,
        scope_issue_code,
        issues,
      );
    }
    Expression::In { expression, values } => {
      validate_expression(
        expression,
        &format!("{location}.expression"),
        sources,
        parameters,
        allowed_sources,
        scope_issue_code,
        issues,
      );
      if values.is_empty() {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyExpressionGroup,
          format!("{location}.values"),
          "in expression must contain at least one value",
        ));
      }
      for (value_index, value) in values.iter().enumerate() {
        validate_expression(
          value,
          &format!("{location}.values[{value_index}]"),
          sources,
          parameters,
          allowed_sources,
          scope_issue_code,
          issues,
        );
      }
    }
    Expression::And { expressions } | Expression::Or { expressions } => {
      if expressions.is_empty() {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyExpressionGroup,
          format!("{location}.expressions"),
          "logical expression must contain at least one item",
        ));
      }
      for (expression_index, expression) in expressions.iter().enumerate() {
        validate_expression(
          expression,
          &format!("{location}.expressions[{expression_index}]"),
          sources,
          parameters,
          allowed_sources,
          scope_issue_code,
          issues,
        );
      }
    }
    Expression::Not { expression }
    | Expression::IsNull { expression }
    | Expression::IsNotNull { expression } => validate_expression(
      expression,
      &format!("{location}.expression"),
      sources,
      parameters,
      allowed_sources,
      scope_issue_code,
      issues,
    ),
    Expression::Function { name, arguments } => {
      if name.trim().is_empty() {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyFunctionName,
          format!("{location}.name"),
          "semantic function name must not be empty",
        ));
      }
      for (argument_index, argument) in arguments.iter().enumerate() {
        validate_expression(
          argument,
          &format!("{location}.arguments[{argument_index}]"),
          sources,
          parameters,
          allowed_sources,
          scope_issue_code,
          issues,
        );
      }
    }
  }
}
