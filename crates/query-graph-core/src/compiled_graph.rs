use crate::{
  analysis::{DefinitionAnalysis, DefinitionIndex, GraphTopology, ProjectionAnalysis},
  DefinitionIssues, ExpressionType, FieldDefinition, GraphDefinition, OrderingDefinition,
  ProjectionFieldDefinition, ProjectionPath, RelationDefinition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstraintPhase {
  BeforeAggregation,
  AfterAggregation,
}

#[derive(Debug)]
pub struct CompiledGraph {
  definition: GraphDefinition,
  index: DefinitionIndex,
  topology: GraphTopology,
  projection: ProjectionAnalysis,
  summary: bool,
  constraint_phases: Box<[ConstraintPhase]>,
}

impl CompiledGraph {
  pub(crate) fn try_from_definition(definition: GraphDefinition) -> Result<Self, DefinitionIssues> {
    let analysis = DefinitionAnalysis::build(&definition)?;
    let summary = definition.is_summary();
    let constraint_phases = definition
      .constraints
      .iter()
      .map(|constraint| {
        if constraint.predicate.contains_aggregate() {
          ConstraintPhase::AfterAggregation
        } else {
          ConstraintPhase::BeforeAggregation
        }
      })
      .collect();

    Ok(Self {
      definition,
      index: analysis.index,
      topology: analysis.topology,
      projection: analysis.projection,
      summary,
      constraint_phases,
    })
  }

  pub fn definition(&self) -> &GraphDefinition {
    &self.definition
  }

  pub fn is_summary(&self) -> bool {
    self.summary
  }

  pub(crate) fn dimension_projection_indices(&self) -> &[usize] {
    self.projection.dimension_indices()
  }

  pub(crate) fn constraint_phase(&self, constraint_index: usize) -> ConstraintPhase {
    self.constraint_phases[constraint_index]
  }

  pub(crate) fn projection_at(&self, projection_index: usize) -> &ProjectionFieldDefinition {
    &self.definition.projection.fields[projection_index]
  }

  pub fn root(&self) -> &crate::SourceDefinition {
    &self.definition.sources[self.index.root().expect("validated graph must have a root")]
  }

  pub fn source(&self, key: &str) -> Option<&crate::SourceDefinition> {
    self
      .index
      .source(key)
      .map(|index| &self.definition.sources[index])
  }

  pub(crate) fn source_index(&self, key: &str) -> Option<usize> {
    self.index.source(key)
  }

  pub fn field(&self, source: &str, field: &str) -> Option<&FieldDefinition> {
    let source_index = self.index.source(source)?;
    let field_index = self.index.field(source_index, field)?;
    Some(&self.definition.sources[source_index].fields[field_index])
  }

  pub fn parameter(&self, name: &str) -> Option<&crate::ParameterDefinition> {
    self
      .index
      .parameter(name)
      .map(|index| &self.definition.parameters[index])
  }

  pub fn relation(&self, name: &str) -> Option<&RelationDefinition> {
    self
      .index
      .relation(name)
      .map(|index| &self.definition.relations[index])
  }

  pub(crate) fn relation_index(&self, name: &str) -> Option<usize> {
    self.index.relation(name)
  }

  pub fn ordering(&self, name: &str) -> Option<&OrderingDefinition> {
    self
      .index
      .ordering(name)
      .map(|index| &self.definition.orderings[index])
  }

  pub(crate) fn ordering_index(&self, name: &str) -> Option<usize> {
    self.index.ordering(name)
  }

  pub(crate) fn default_ordering_index(&self) -> Option<usize> {
    self.index.default_ordering()
  }

  pub(crate) fn ordering_at(&self, ordering_index: usize) -> &OrderingDefinition {
    &self.definition.orderings[ordering_index]
  }

  pub fn projection(&self, path: &str) -> Option<&ProjectionFieldDefinition> {
    let path = ProjectionPath::parse(path);
    self
      .index
      .projection(&path)
      .map(|index| &self.definition.projection.fields[index])
  }

  pub fn projection_type(&self, path: &str) -> Option<ExpressionType> {
    let path = ProjectionPath::parse(path);
    self
      .index
      .projection(&path)
      .map(|index| self.projection.expression_type(index))
  }

  pub(crate) fn projection_index(&self, path: &ProjectionPath) -> Option<usize> {
    self.index.projection(path)
  }

  pub(crate) fn projection_type_at(&self, projection_index: usize) -> ExpressionType {
    self.projection.expression_type(projection_index)
  }

  pub(crate) fn projection_relation_path_indices(&self, projection_index: usize) -> &[usize] {
    self.projection.relation_path(projection_index)
  }

  pub(crate) fn selected_projection_object_indices(
    &self,
    projection_indices: &[usize],
  ) -> Vec<usize> {
    let mut object_indices: Vec<_> = self
      .definition
      .projection
      .objects
      .iter()
      .enumerate()
      .filter_map(|(object_index, object)| {
        projection_indices
          .iter()
          .map(|index| &self.definition.projection.fields[*index].path)
          .any(|path| path.len() > object.path.len() && path.starts_with(&object.path))
          .then_some(object_index)
      })
      .collect();

    object_indices.sort_by(|left, right| {
      self.definition.projection.objects[*right]
        .path
        .len()
        .cmp(&self.definition.projection.objects[*left].path.len())
        .then_with(|| left.cmp(right))
    });
    object_indices
  }

  pub(crate) fn projection_object_relation_path_indices(&self, object_index: usize) -> &[usize] {
    self.projection.object_relation_path(object_index)
  }

  pub fn outgoing_relations(
    &self,
    source: &str,
  ) -> Option<impl Iterator<Item = &RelationDefinition>> {
    let source_index = self.index.source(source)?;
    Some(
      self
        .topology
        .outgoing_relations(source_index)
        .iter()
        .map(|index| &self.definition.relations[*index]),
    )
  }

  pub(crate) fn relation_is_effectively_required(&self, relation_index: usize) -> bool {
    self
      .topology
      .relation_is_effectively_required(relation_index)
  }

  pub(crate) fn relation_path_indices(&self, source: &str) -> Option<&[usize]> {
    self
      .index
      .source(source)
      .and_then(|source| self.topology.relation_path(source))
  }

  pub(crate) fn relation_path_indices_between(&self, from: &str, to: &str) -> Option<&[usize]> {
    let from = self.index.source(from)?;
    let to = self.index.source(to)?;
    self.topology.relation_path_between(from, to)
  }

  pub fn relation_path(&self, source: &str) -> Option<impl Iterator<Item = &RelationDefinition>> {
    Some(
      self
        .relation_path_indices(source)?
        .iter()
        .map(|index| &self.definition.relations[*index]),
    )
  }
}
