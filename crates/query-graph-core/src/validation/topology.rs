use std::collections::HashSet;

use crate::{
  analysis::{DefinitionIndex, GraphTopology},
  GraphDefinition,
};

use super::{DefinitionIssue, DefinitionIssueCode};

pub(super) fn validate(
  definition: &GraphDefinition,
  index: &DefinitionIndex,
  topology: &GraphTopology,
  issues: &mut Vec<DefinitionIssue>,
) {
  for (relation_index, relation) in definition.relations.iter().enumerate() {
    if index.source(&relation.from).is_none() || index.source(&relation.to).is_none() {
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
  }

  validate_unique_paths(definition, index, topology, issues);
  validate_reachability(definition, index, topology, issues);
}

fn validate_unique_paths(
  definition: &GraphDefinition,
  index: &DefinitionIndex,
  topology: &GraphTopology,
  issues: &mut Vec<DefinitionIssue>,
) {
  let mut visited = HashSet::new();
  for source in &definition.sources {
    if source.key == definition.root || !visited.insert(source.key.as_str()) {
      continue;
    }
    let Some(source_index) = index.source(&source.key) else {
      continue;
    };
    let relations = topology.incoming_relations(source_index);
    if relations.len() <= 1 {
      continue;
    }

    let names: Vec<_> = relations
      .iter()
      .map(|relation| definition.relations[*relation].name.as_str())
      .collect();
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::AmbiguousSourcePath,
      format!("sources.{}", source.key),
      format!(
        "source {:?} has multiple incoming relations: {names:?}",
        source.key
      ),
    ));
  }
}

fn validate_reachability(
  definition: &GraphDefinition,
  index: &DefinitionIndex,
  topology: &GraphTopology,
  issues: &mut Vec<DefinitionIssue>,
) {
  if index.root().is_none() {
    return;
  }

  for (source_index, source) in definition.sources.iter().enumerate() {
    let reachable = index
      .source(&source.key)
      .is_some_and(|source| topology.source_is_reachable(source));
    if !reachable {
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
