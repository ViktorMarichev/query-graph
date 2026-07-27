use std::collections::HashMap;

use crate::{validation, DefinitionIssues, FieldDefinition, GraphDefinition, RelationDefinition};

#[derive(Debug, Clone)]
pub struct CompiledGraph {
  definition: GraphDefinition,
  root_index: usize,
  source_index: HashMap<String, usize>,
  field_index: Vec<HashMap<String, usize>>,
  parameter_index: HashMap<String, usize>,
  relation_index: HashMap<String, usize>,
  outgoing_relations: Vec<Vec<usize>>,
}

impl CompiledGraph {
  pub(crate) fn try_from_definition(definition: GraphDefinition) -> Result<Self, DefinitionIssues> {
    validation::validate(&definition)?;

    let source_index: HashMap<_, _> = definition
      .sources
      .iter()
      .enumerate()
      .map(|(index, source)| (source.key.clone(), index))
      .collect();
    let root_index = source_index[&definition.root];

    let field_index = definition
      .sources
      .iter()
      .map(|source| {
        source
          .fields
          .iter()
          .enumerate()
          .map(|(index, field)| (field.name.clone(), index))
          .collect()
      })
      .collect();

    let parameter_index = definition
      .parameters
      .iter()
      .enumerate()
      .map(|(index, parameter)| (parameter.name.clone(), index))
      .collect();

    let relation_index: HashMap<_, _> = definition
      .relations
      .iter()
      .enumerate()
      .map(|(index, relation)| (relation.name.clone(), index))
      .collect();

    let mut outgoing_relations = vec![Vec::new(); definition.sources.len()];
    for (relation_index, relation) in definition.relations.iter().enumerate() {
      outgoing_relations[source_index[&relation.from]].push(relation_index);
    }

    Ok(Self {
      definition,
      root_index,
      source_index,
      field_index,
      parameter_index,
      relation_index,
      outgoing_relations,
    })
  }

  pub fn definition(&self) -> &GraphDefinition {
    &self.definition
  }

  pub fn root(&self) -> &crate::SourceDefinition {
    &self.definition.sources[self.root_index]
  }

  pub fn source(&self, key: &str) -> Option<&crate::SourceDefinition> {
    self
      .source_index
      .get(key)
      .map(|index| &self.definition.sources[*index])
  }

  pub fn field(&self, source: &str, field: &str) -> Option<&FieldDefinition> {
    let source_index = *self.source_index.get(source)?;
    let field_index = *self.field_index[source_index].get(field)?;
    Some(&self.definition.sources[source_index].fields[field_index])
  }

  pub fn parameter(&self, name: &str) -> Option<&crate::ParameterDefinition> {
    self
      .parameter_index
      .get(name)
      .map(|index| &self.definition.parameters[*index])
  }

  pub fn relation(&self, name: &str) -> Option<&RelationDefinition> {
    self
      .relation_index
      .get(name)
      .map(|index| &self.definition.relations[*index])
  }

  pub fn outgoing_relations(
    &self,
    source: &str,
  ) -> Option<impl Iterator<Item = &RelationDefinition>> {
    let source_index = *self.source_index.get(source)?;
    Some(
      self.outgoing_relations[source_index]
        .iter()
        .map(|index| &self.definition.relations[*index]),
    )
  }
}
