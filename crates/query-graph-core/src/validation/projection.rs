use std::collections::{HashMap, HashSet};

use crate::{GraphDefinition, ProjectionFieldDefinition};

use super::{DefinitionIssue, DefinitionIssueCode, DefinitionIssues};

pub(super) fn infer_relation_paths(
  definition: &GraphDefinition,
  source_index: &HashMap<String, usize>,
  relation_paths: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, DefinitionIssues> {
  let mut issues = Vec::new();
  let mut projection_paths = Vec::with_capacity(definition.projection.fields.len());

  for (projection_index, projection) in definition.projection.fields.iter().enumerate() {
    let referenced_sources = referenced_sources(projection);
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
            format!("projection.fields[{projection_index}].expression"),
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

    projection_paths.push(inferred_path.cloned().unwrap_or_default());
  }

  if issues.is_empty() {
    Ok(projection_paths)
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
  projection: &ProjectionFieldDefinition,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
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

fn referenced_sources(projection: &ProjectionFieldDefinition) -> Vec<&str> {
  let mut sources = Vec::new();
  projection.expression.for_each_field(&mut |source, _| {
    if !sources.contains(&source) {
      sources.push(source);
    }
  });
  sources
}
