use std::collections::HashSet;

use crate::{analysis::DefinitionIndex, Expression, GraphDefinition};

use super::{DefinitionIssue, DefinitionIssueCode};

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
  index: &DefinitionIndex,
  expression: &Expression,
  location: &str,
  issues: &mut Vec<DefinitionIssue>,
) {
  let mut referenced_fields = HashSet::new();
  expression.for_each_field(&mut |source, field| {
    referenced_fields.insert((source, field));
  });

  for (source, field) in referenced_fields {
    let Some(source_index) = index.source(source) else {
      continue;
    };
    let Some(field_index) = index.field(source_index, field) else {
      continue;
    };
    if !definition.sources[source_index].fields[field_index].selectable {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::HiddenProjectionField,
        location,
        format!("field {source:?}.{field:?} is internal and cannot be exposed"),
      ));
    }
  }
}
