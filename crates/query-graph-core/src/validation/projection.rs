use std::collections::{HashMap, HashSet};

use crate::{Expression, GraphDefinition};

use super::{DefinitionIssue, DefinitionIssueCode, DefinitionIssues};

pub(super) fn infer_relation_paths(
  definition: &GraphDefinition,
  source_index: &HashMap<String, usize>,
  relation_paths: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, DefinitionIssues> {
  infer_expression_relation_paths(
    definition,
    definition
      .projection
      .fields
      .iter()
      .enumerate()
      .map(|(index, projection)| {
        (
          format!("projection.fields[{index}].expression"),
          &projection.expression,
        )
      }),
    source_index,
    relation_paths,
  )
}

pub(super) fn infer_object_relation_paths(
  definition: &GraphDefinition,
  source_index: &HashMap<String, usize>,
  relation_paths: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, DefinitionIssues> {
  infer_expression_relation_paths(
    definition,
    definition
      .projection
      .objects
      .iter()
      .enumerate()
      .map(|(index, object)| {
        (
          format!("projection.objects[{index}].presence"),
          &object.presence,
        )
      }),
    source_index,
    relation_paths,
  )
}

fn infer_expression_relation_paths<'a>(
  definition: &GraphDefinition,
  expressions: impl Iterator<Item = (String, &'a Expression)>,
  source_index: &HashMap<String, usize>,
  relation_paths: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, DefinitionIssues> {
  let mut issues = Vec::new();
  let mut expression_paths = Vec::new();

  for (location, expression) in expressions {
    let referenced_sources = referenced_sources(expression);
    let mut inferred_path: Option<&Vec<usize>> = None;
    let mut inferred_source: Option<&str> = None;

    for source in referenced_sources {
      let Some(source_index) = source_index.get(source) else {
        continue;
      };
      let candidate_path = &relation_paths[*source_index];

      match inferred_path {
        None => {
          inferred_path = Some(candidate_path);
          inferred_source = Some(source);
        }
        Some(current_path) if candidate_path.starts_with(current_path) => {
          inferred_path = Some(candidate_path);
          inferred_source = Some(source);
        }
        Some(current_path) if current_path.starts_with(candidate_path) => {}
        Some(_) => {
          issues.push(DefinitionIssue::new(
            DefinitionIssueCode::ProjectionExpressionScope,
            location.clone(),
            format!(
              "projection expression sources {:?} and {:?} are on different relation branches",
              inferred_source.unwrap_or(definition.root.as_str()),
              source
            ),
          ));
          break;
        }
      }
    }

    expression_paths.push(inferred_path.cloned().unwrap_or_default());
  }

  if issues.is_empty() {
    Ok(expression_paths)
  } else {
    Err(DefinitionIssues::from_vec(issues))
  }
}

pub(super) fn paths_conflict(left: &[String], right: &[String]) -> bool {
  if left.is_empty() || right.is_empty() || left.len() == right.len() {
    return false;
  }

  if left.len() < right.len() {
    right.starts_with(left)
  } else {
    left.starts_with(right)
  }
}

pub(super) fn validate_visibility(
  definition: &GraphDefinition,
  expression: &Expression,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
  let mut referenced_fields = HashSet::new();
  expression.for_each_field(&mut |source, field| {
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
        location,
        format!("field {source:?}.{field:?} is internal and cannot be exposed"),
      ));
    }
  }
}

fn referenced_sources(expression: &Expression) -> Vec<&str> {
  let mut sources = Vec::new();
  expression.for_each_field(&mut |source, _| {
    if !sources.contains(&source) {
      sources.push(source);
    }
  });
  sources
}
