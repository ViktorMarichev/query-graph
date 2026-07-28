use std::collections::{HashMap, HashSet, VecDeque};

use crate::GraphDefinition;

use super::{DefinitionIssue, DefinitionIssueCode};

pub(super) fn validate(
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

  validate_unique_paths(&definition.root, incoming, issues);
  validate_reachability(definition, sources, issues);
}

fn validate_unique_paths(
  root: &str,
  incoming: HashMap<&str, Vec<(usize, &str)>>,
  issues: &mut Vec<DefinitionIssue>,
) {
  for (source, relations) in incoming {
    if source == root || relations.len() <= 1 {
      continue;
    }

    let names: Vec<_> = relations.iter().map(|(_, name)| *name).collect();
    issues.push(DefinitionIssue::new(
      DefinitionIssueCode::AmbiguousSourcePath,
      format!("sources.{source}"),
      format!("source {source:?} has multiple incoming relations: {names:?}"),
    ));
  }
}

fn validate_reachability(
  definition: &GraphDefinition,
  sources: &HashMap<String, HashSet<String>>,
  issues: &mut Vec<DefinitionIssue>,
) {
  if !sources.contains_key(&definition.root) {
    return;
  }

  let reachable = reachable_sources(definition, sources);
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

pub(super) fn infer_source_scopes(
  definition: &GraphDefinition,
  sources: &HashMap<String, HashSet<String>>,
) -> HashMap<String, HashSet<String>> {
  if !sources.contains_key(&definition.root) {
    return HashMap::new();
  }

  let mut scopes = HashMap::from([(
    definition.root.clone(),
    HashSet::from([definition.root.clone()]),
  )]);
  let mut queue = VecDeque::from([definition.root.clone()]);

  while let Some(source) = queue.pop_front() {
    let Some(parent_scope) = scopes.get(&source).cloned() else {
      continue;
    };

    for relation in definition
      .relations
      .iter()
      .filter(|relation| relation.from == source && sources.contains_key(&relation.to))
    {
      if scopes.contains_key(&relation.to) {
        continue;
      }

      let mut scope = parent_scope.clone();
      scope.insert(relation.to.clone());
      scopes.insert(relation.to.clone(), scope);
      queue.push_back(relation.to.clone());
    }
  }

  scopes
}

fn reachable_sources(
  definition: &GraphDefinition,
  sources: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
  let mut reachable = HashSet::from([definition.root.clone()]);
  let mut queue = VecDeque::from([definition.root.clone()]);

  while let Some(source) = queue.pop_front() {
    for relation in definition
      .relations
      .iter()
      .filter(|relation| relation.from == source)
    {
      if sources.contains_key(&relation.to) && reachable.insert(relation.to.clone()) {
        queue.push_back(relation.to.clone());
      }
    }
  }

  reachable
}
