mod index;
mod projection;
mod topology;

use crate::{
  type_validation::ExpressionTypeChecker, validation, DefinitionIssues, GraphDefinition,
};

pub(crate) use index::DefinitionIndex;
pub(crate) use projection::ProjectionAnalysis;
pub(crate) use topology::GraphTopology;

#[derive(Debug)]
pub(crate) struct DefinitionAnalysis {
  pub(crate) index: DefinitionIndex,
  pub(crate) topology: GraphTopology,
  pub(crate) projection: ProjectionAnalysis,
}

impl DefinitionAnalysis {
  pub(crate) fn build(definition: &GraphDefinition) -> Result<Self, DefinitionIssues> {
    let index = DefinitionIndex::build(definition);
    let topology = GraphTopology::build(definition, &index);
    validation::validate(definition, &index, &topology)?;
    let expression_types = ExpressionTypeChecker::new(definition, &index, &topology).analyze()?;
    let projection = ProjectionAnalysis::build(definition, &index, &topology, expression_types)?;

    Ok(Self {
      index,
      topology,
      projection,
    })
  }
}
