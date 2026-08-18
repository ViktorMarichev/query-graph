use std::collections::HashMap;

use crate::{GraphDefinition, ProjectionPath};

#[derive(Debug)]
pub(crate) struct DefinitionIndex {
  root: Option<usize>,
  sources: HashMap<String, usize>,
  fields: Vec<HashMap<String, usize>>,
  parameters: HashMap<String, usize>,
  relations: HashMap<String, usize>,
  orderings: HashMap<String, usize>,
  projections: HashMap<ProjectionPath, usize>,
  default_ordering: Option<usize>,
}

impl DefinitionIndex {
  pub(crate) fn build(definition: &GraphDefinition) -> Self {
    let mut sources = HashMap::new();
    let mut fields = Vec::with_capacity(definition.sources.len());

    for (source_index, source) in definition.sources.iter().enumerate() {
      if !source.key.trim().is_empty() {
        sources.entry(source.key.clone()).or_insert(source_index);
      }

      let mut source_fields = HashMap::new();
      for (field_index, field) in source.fields.iter().enumerate() {
        if !field.name.trim().is_empty() {
          source_fields
            .entry(field.name.clone())
            .or_insert(field_index);
        }
      }
      fields.push(source_fields);
    }

    let mut parameters = HashMap::new();
    for (index, parameter) in definition.parameters.iter().enumerate() {
      if !parameter.name.trim().is_empty() {
        parameters.insert(parameter.name.clone(), index);
      }
    }

    let mut relations = HashMap::new();
    for (index, relation) in definition.relations.iter().enumerate() {
      if !relation.name.trim().is_empty() {
        relations.entry(relation.name.clone()).or_insert(index);
      }
    }

    let mut orderings = HashMap::new();
    for (index, ordering) in definition.orderings.iter().enumerate() {
      if !ordering.name.trim().is_empty() {
        orderings.entry(ordering.name.clone()).or_insert(index);
      }
    }

    let projections = definition
      .projection
      .fields
      .iter()
      .enumerate()
      .map(|(index, field)| (ProjectionPath::from_segments(&field.path), index))
      .collect();
    let default_ordering = definition
      .orderings
      .iter()
      .position(|ordering| ordering.selected_by_default);
    let root = sources.get(&definition.root).copied();

    Self {
      root,
      sources,
      fields,
      parameters,
      relations,
      orderings,
      projections,
      default_ordering,
    }
  }

  pub(crate) fn root(&self) -> Option<usize> {
    self.root
  }

  pub(crate) fn source(&self, key: &str) -> Option<usize> {
    self.sources.get(key).copied()
  }

  pub(crate) fn field(&self, source: usize, name: &str) -> Option<usize> {
    self.fields.get(source)?.get(name).copied()
  }

  pub(crate) fn parameter(&self, name: &str) -> Option<usize> {
    self.parameters.get(name).copied()
  }

  pub(crate) fn relation(&self, name: &str) -> Option<usize> {
    self.relations.get(name).copied()
  }

  pub(crate) fn ordering(&self, name: &str) -> Option<usize> {
    self.orderings.get(name).copied()
  }

  pub(crate) fn projection(&self, path: &ProjectionPath) -> Option<usize> {
    self.projections.get(path).copied()
  }

  pub(crate) fn default_ordering(&self) -> Option<usize> {
    self.default_ordering
  }
}
