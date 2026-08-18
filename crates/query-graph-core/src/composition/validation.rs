use serde_json::Value;

use crate::{MappedQueryGraph, OperationIssueCode, ParameterShape, QueryOperation};

use super::model::{
  BatchRelation, BatchRelationDefinition, CompositionIssue, CompositionIssueCode, CompositionIssues,
};

pub(super) fn validate_relation(
  root: &MappedQueryGraph,
  relations: &[BatchRelation],
  graph: &MappedQueryGraph,
  definition: &BatchRelationDefinition,
) -> Result<(), CompositionIssues> {
  let relation_index = relations.len();
  let location = format!("relations[{relation_index}]");
  let mut issues = Vec::new();

  if definition.name.trim().is_empty() {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::EmptyRelationName,
      format!("{location}.name"),
      "batch relation name must not be empty",
    ));
  } else if definition.name.contains('.') {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::InvalidRelationName,
      format!("{location}.name"),
      "batch relation name must be a single projection path segment",
    ));
  }

  if relations
    .iter()
    .any(|relation| relation.definition.name == definition.name)
  {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::DuplicateRelationName,
      format!("{location}.name"),
      format!(
        "batch relation {:?} is defined more than once",
        definition.name
      ),
    ));
  }

  let namespace = format!("{}.", definition.name);
  if root
    .graph()
    .definition()
    .projection
    .fields
    .iter()
    .map(|field| field.path.join("."))
    .any(|path| path == definition.name || path.starts_with(&namespace))
  {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::ConflictingProjectionPath,
      format!("{location}.name"),
      format!(
        "batch relation {:?} conflicts with a root projection path",
        definition.name
      ),
    ));
  }

  let parent_type = root.graph().projection_type(&definition.from);
  if parent_type.is_none() {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::UnknownParentKey,
      format!("{location}.from"),
      format!("projection field {:?} is not defined", definition.from),
    ));
  }

  let child_type = graph.graph().projection_type(&definition.to);
  if child_type.is_none() {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::UnknownChildKey,
      format!("{location}.to"),
      format!("projection field {:?} is not defined", definition.to),
    ));
  }

  let parameter = graph.graph().parameter(&definition.parameter);
  match parameter {
    None => issues.push(CompositionIssue::new(
      CompositionIssueCode::UnknownKeyParameter,
      format!("{location}.parameter"),
      format!("parameter {:?} is not defined", definition.parameter),
    )),
    Some(parameter) if parameter.shape != ParameterShape::List => {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::KeyParameterNotList,
        format!("{location}.parameter"),
        "batch key parameter must have list shape",
      ));
    }
    Some(_) => {}
  }

  if let (Some(parent_type), Some(child_type), Some(parameter)) =
    (parent_type, child_type, parameter)
  {
    let types = [
      parent_type.scalar_type,
      child_type.scalar_type,
      parameter.scalar_type,
    ];
    if types.iter().any(|scalar_type| *scalar_type != types[0]) {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::IncompatibleKeyTypes,
        location.clone(),
        "parent key, child key, and key parameter must have the same scalar type",
      ));
    }
  }

  if definition.parameters.contains_key(&definition.parameter) {
    issues.push(CompositionIssue::new(
      CompositionIssueCode::KeyParameterIsStatic,
      format!("{location}.parameters.{}", definition.parameter),
      "the batch key parameter is supplied automatically",
    ));
  }

  validate_child_operation(graph, definition, &location, &mut issues);

  if issues.is_empty() {
    Ok(())
  } else {
    Err(CompositionIssues::from_vec(issues))
  }
}

fn validate_child_operation(
  graph: &MappedQueryGraph,
  definition: &BatchRelationDefinition,
  location: &str,
  issues: &mut Vec<CompositionIssue>,
) {
  let selected_path = graph
    .graph()
    .projection(&definition.to)
    .map(|_| definition.to.clone())
    .or_else(|| {
      graph
        .graph()
        .definition()
        .projection
        .fields
        .first()
        .map(|field| field.path.join("."))
    });
  let Some(selected_path) = selected_path else {
    return;
  };

  let mut parameters = definition.parameters.clone();
  if graph
    .graph()
    .parameter(&definition.parameter)
    .is_some_and(|parameter| parameter.shape == ParameterShape::List)
  {
    parameters.insert(definition.parameter.clone(), Value::Array(Vec::new()));
  }

  let operation = QueryOperation {
    select: Some(vec![selected_path]),
    ordering: definition.ordering.clone(),
    parameters,
    ..QueryOperation::default()
  };

  let Err(operation_issues) = operation.validate(graph.graph()) else {
    return;
  };

  let key_location = format!("parameters.{}", definition.parameter);
  for issue in operation_issues.into_vec() {
    if issue.location == key_location || issue.location.starts_with(&format!("{key_location}[")) {
      continue;
    }

    let (code, issue_location) = match issue.code {
      OperationIssueCode::UnknownOrdering => (
        CompositionIssueCode::UnknownChildOrdering,
        format!("{location}.ordering"),
      ),
      OperationIssueCode::UnknownParameter => (
        CompositionIssueCode::UnknownStaticParameter,
        prefix_operation_location(location, &issue.location),
      ),
      OperationIssueCode::MissingParameter => (
        CompositionIssueCode::MissingStaticParameter,
        prefix_operation_location(location, &issue.location),
      ),
      OperationIssueCode::InvalidParameterType => (
        CompositionIssueCode::InvalidStaticParameterType,
        prefix_operation_location(location, &issue.location),
      ),
      _ => continue,
    };
    issues.push(CompositionIssue::new(code, issue_location, issue.message));
  }
}

fn prefix_operation_location(relation_location: &str, operation_location: &str) -> String {
  operation_location.strip_prefix("parameters").map_or_else(
    || format!("{relation_location}.{operation_location}"),
    |suffix| format!("{relation_location}.parameters{suffix}"),
  )
}
