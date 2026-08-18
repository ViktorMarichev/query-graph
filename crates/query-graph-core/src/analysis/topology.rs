use std::collections::VecDeque;

use crate::GraphDefinition;

use super::DefinitionIndex;

#[derive(Debug)]
pub(crate) struct GraphTopology {
  outgoing_relations: Vec<Box<[usize]>>,
  incoming_relations: Vec<Box<[usize]>>,
  relation_paths: Vec<Option<Box<[usize]>>>,
  source_nullable: Box<[bool]>,
  effective_relation_required: Box<[bool]>,
}

impl GraphTopology {
  pub(crate) fn build(definition: &GraphDefinition, index: &DefinitionIndex) -> Self {
    let mut outgoing_relations = vec![Vec::new(); definition.sources.len()];
    let mut incoming_relations = vec![Vec::new(); definition.sources.len()];

    for (relation_index, relation) in definition.relations.iter().enumerate() {
      let (Some(from), Some(to)) = (index.source(&relation.from), index.source(&relation.to))
      else {
        continue;
      };
      outgoing_relations[from].push(relation_index);
      incoming_relations[to].push(relation_index);
    }

    let mut relation_paths: Vec<Option<Vec<usize>>> = vec![None; definition.sources.len()];
    if let Some(root) = index.root() {
      relation_paths[root] = Some(Vec::new());
      let mut queue = VecDeque::from([root]);

      while let Some(source) = queue.pop_front() {
        let parent_path = relation_paths[source]
          .as_ref()
          .expect("queued source must have a relation path")
          .clone();

        for relation_index in &outgoing_relations[source] {
          let relation = &definition.relations[*relation_index];
          let Some(target) = index.source(&relation.to) else {
            continue;
          };
          if relation_paths[target].is_some() {
            continue;
          }

          let mut path = parent_path.clone();
          path.push(*relation_index);
          relation_paths[target] = Some(path);
          queue.push_back(target);
        }
      }
    }

    let source_nullable = relation_paths
      .iter()
      .map(|path| {
        path.as_deref().is_some_and(|path| {
          path
            .iter()
            .any(|index| !definition.relations[*index].required)
        })
      })
      .collect();
    let effective_relation_required = definition
      .relations
      .iter()
      .map(|relation| {
        index
          .source(&relation.to)
          .and_then(|source| relation_paths[source].as_deref())
          .is_some_and(|path| {
            path
              .iter()
              .all(|index| definition.relations[*index].required)
          })
      })
      .collect();

    Self {
      outgoing_relations: outgoing_relations
        .into_iter()
        .map(Vec::into_boxed_slice)
        .collect(),
      incoming_relations: incoming_relations
        .into_iter()
        .map(Vec::into_boxed_slice)
        .collect(),
      relation_paths: relation_paths
        .into_iter()
        .map(|path| path.map(Vec::into_boxed_slice))
        .collect(),
      source_nullable,
      effective_relation_required,
    }
  }

  pub(crate) fn outgoing_relations(&self, source: usize) -> &[usize] {
    &self.outgoing_relations[source]
  }

  pub(crate) fn incoming_relations(&self, source: usize) -> &[usize] {
    &self.incoming_relations[source]
  }

  pub(crate) fn relation_path(&self, source: usize) -> Option<&[usize]> {
    self.relation_paths[source].as_deref()
  }

  pub(crate) fn relation_path_between(&self, from: usize, to: usize) -> Option<&[usize]> {
    let from_path = self.relation_path(from)?;
    let to_path = self.relation_path(to)?;
    to_path
      .starts_with(from_path)
      .then_some(&to_path[from_path.len()..])
  }

  pub(crate) fn source_is_reachable(&self, source: usize) -> bool {
    self.relation_paths[source].is_some()
  }

  pub(crate) fn source_is_nullable(&self, source: usize) -> bool {
    self.source_nullable[source]
  }

  pub(crate) fn relation_is_effectively_required(&self, relation: usize) -> bool {
    self.effective_relation_required[relation]
  }
}
