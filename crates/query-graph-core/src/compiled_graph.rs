use std::collections::HashMap;

use crate::{
  type_validation, validation, DefinitionIssues, ExpressionType, FieldDefinition, GraphDefinition,
  ProjectionFieldDefinition, ProjectionPath, RelationDefinition,
};

#[derive(Debug)]
struct CompiledProjection {
  expression_type: ExpressionType,
  relation_path: Box<[usize]>,
}

#[derive(Debug)]
pub struct CompiledGraph {
  definition: GraphDefinition,
  root_index: usize,
  source_index: HashMap<String, usize>,
  field_index: Vec<HashMap<String, usize>>,
  parameter_index: HashMap<String, usize>,
  relation_index: HashMap<String, usize>,
  projection_index: HashMap<ProjectionPath, usize>,
  projections: Vec<CompiledProjection>,
  outgoing_relations: Vec<Vec<usize>>,
  relation_paths: Vec<Vec<usize>>,
  effective_relation_required: Vec<bool>,
}

impl CompiledGraph {
  pub(crate) fn try_from_definition(definition: GraphDefinition) -> Result<Self, DefinitionIssues> {
    validation::validate(&definition)?;
    let projection_types = type_validation::analyze(&definition)?;

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

    let projection_index = definition
      .projection
      .fields
      .iter()
      .enumerate()
      .map(|(index, field)| (ProjectionPath::from_segments(&field.path), index))
      .collect();

    let mut outgoing_relations = vec![Vec::new(); definition.sources.len()];
    let mut incoming_relations = vec![None; definition.sources.len()];
    for (relation_index, relation) in definition.relations.iter().enumerate() {
      let from_index = source_index[&relation.from];
      let to_index = source_index[&relation.to];
      outgoing_relations[from_index].push(relation_index);
      incoming_relations[to_index] = Some(relation_index);
    }

    let relation_paths =
      build_relation_paths(&definition, &source_index, root_index, &incoming_relations);
    let projection_relation_paths =
      validation::infer_projection_relation_paths(&definition, &source_index, &relation_paths)?;
    let projections = projection_types
      .into_iter()
      .zip(projection_relation_paths)
      .map(|(expression_type, relation_path)| CompiledProjection {
        expression_type,
        relation_path: relation_path.into_boxed_slice(),
      })
      .collect();
    let effective_relation_required = definition
      .relations
      .iter()
      .map(|relation| {
        relation_paths[source_index[&relation.to]]
          .iter()
          .all(|index| definition.relations[*index].required)
      })
      .collect();

    Ok(Self {
      definition,
      root_index,
      source_index,
      field_index,
      parameter_index,
      relation_index,
      projection_index,
      projections,
      outgoing_relations,
      relation_paths,
      effective_relation_required,
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

  pub(crate) fn source_index(&self, key: &str) -> Option<usize> {
    self.source_index.get(key).copied()
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

  pub(crate) fn relation_index(&self, name: &str) -> Option<usize> {
    self.relation_index.get(name).copied()
  }

  pub fn projection(&self, path: &str) -> Option<&ProjectionFieldDefinition> {
    let path = ProjectionPath::parse(path);
    self
      .projection_index
      .get(&path)
      .map(|index| &self.definition.projection.fields[*index])
  }

  pub fn projection_type(&self, path: &str) -> Option<ExpressionType> {
    let path = ProjectionPath::parse(path);
    self
      .projection_index
      .get(&path)
      .map(|index| self.projections[*index].expression_type)
  }

  pub(crate) fn projection_index(&self, path: &ProjectionPath) -> Option<usize> {
    self.projection_index.get(path).copied()
  }

  pub(crate) fn projection_type_at(&self, projection_index: usize) -> ExpressionType {
    self.projections[projection_index].expression_type
  }

  pub(crate) fn projection_relation_path_indices(&self, projection_index: usize) -> &[usize] {
    &self.projections[projection_index].relation_path
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

  pub(crate) fn relation_is_effectively_required(&self, relation_index: usize) -> bool {
    self.effective_relation_required[relation_index]
  }

  pub(crate) fn relation_path_indices(&self, source: &str) -> Option<&[usize]> {
    let source_index = *self.source_index.get(source)?;
    Some(&self.relation_paths[source_index])
  }

  pub fn relation_path(&self, source: &str) -> Option<impl Iterator<Item = &RelationDefinition>> {
    let source_index = *self.source_index.get(source)?;
    Some(
      self.relation_paths[source_index]
        .iter()
        .map(|index| &self.definition.relations[*index]),
    )
  }
}

fn build_relation_paths(
  definition: &GraphDefinition,
  source_index: &HashMap<String, usize>,
  root_index: usize,
  incoming_relations: &[Option<usize>],
) -> Vec<Vec<usize>> {
  let mut paths = vec![Vec::new(); definition.sources.len()];

  for (source_index_value, path) in paths.iter_mut().enumerate() {
    if source_index_value == root_index {
      continue;
    }

    let mut current = source_index_value;
    let mut reversed_path = Vec::new();
    while current != root_index {
      let relation_index = incoming_relations[current]
        .expect("validated graph source must have one incoming relation");
      reversed_path.push(relation_index);
      let relation = &definition.relations[relation_index];
      current = source_index[&relation.from];
    }
    reversed_path.reverse();
    *path = reversed_path;
  }

  paths
}
