use crate::{
  DefinitionIssue, DefinitionIssueCode, DefinitionIssues, Expression, ExpressionType,
  GraphDefinition, ProjectionFieldRole,
};

use super::{DefinitionIndex, GraphTopology};

#[derive(Debug)]
struct AnalyzedProjection {
  expression_type: ExpressionType,
  relation_path: Box<[usize]>,
}

#[derive(Debug)]
struct AnalyzedProjectionObject {
  relation_path: Box<[usize]>,
}

#[derive(Debug)]
pub(crate) struct ProjectionAnalysis {
  fields: Box<[AnalyzedProjection]>,
  objects: Box<[AnalyzedProjectionObject]>,
  dimension_indices: Box<[usize]>,
}

impl ProjectionAnalysis {
  pub(crate) fn build(
    definition: &GraphDefinition,
    index: &DefinitionIndex,
    topology: &GraphTopology,
    expression_types: Vec<ExpressionType>,
  ) -> Result<Self, DefinitionIssues> {
    let field_paths = infer_expression_paths(
      definition,
      definition
        .projection
        .fields
        .iter()
        .enumerate()
        .map(|(index, projection)| {
          (
            format!("projection.fields[{index}].expression"),
            &projection.expression,
          )
        }),
      index,
      topology,
    )?;
    let object_paths = infer_expression_paths(
      definition,
      definition
        .projection
        .objects
        .iter()
        .enumerate()
        .map(|(index, object)| {
          (
            format!("projection.objects[{index}].presence"),
            &object.presence,
          )
        }),
      index,
      topology,
    )?;

    let fields = expression_types
      .into_iter()
      .zip(field_paths)
      .map(|(expression_type, relation_path)| AnalyzedProjection {
        expression_type,
        relation_path,
      })
      .collect();
    let objects = object_paths
      .into_iter()
      .map(|relation_path| AnalyzedProjectionObject { relation_path })
      .collect();
    let dimension_indices = definition
      .projection
      .fields
      .iter()
      .enumerate()
      .filter_map(|(index, field)| (field.role == ProjectionFieldRole::Dimension).then_some(index))
      .collect();

    Ok(Self {
      fields,
      objects,
      dimension_indices,
    })
  }

  pub(crate) fn expression_type(&self, projection: usize) -> ExpressionType {
    self.fields[projection].expression_type
  }

  pub(crate) fn relation_path(&self, projection: usize) -> &[usize] {
    &self.fields[projection].relation_path
  }

  pub(crate) fn object_relation_path(&self, object: usize) -> &[usize] {
    &self.objects[object].relation_path
  }

  pub(crate) fn dimension_indices(&self) -> &[usize] {
    &self.dimension_indices
  }
}

fn infer_expression_paths<'a>(
  definition: &GraphDefinition,
  expressions: impl Iterator<Item = (String, &'a Expression)>,
  index: &DefinitionIndex,
  topology: &GraphTopology,
) -> Result<Vec<Box<[usize]>>, DefinitionIssues> {
  let mut issues = Vec::new();
  let mut expression_paths = Vec::new();

  for (location, expression) in expressions {
    let referenced_sources = referenced_sources(expression);
    let mut inferred_path: Option<&[usize]> = None;
    let mut inferred_source: Option<&str> = None;

    for source in referenced_sources {
      let Some(source_index) = index.source(source) else {
        continue;
      };
      let Some(candidate_path) = topology.relation_path(source_index) else {
        continue;
      };

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
            location.clone(),
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

    expression_paths.push(inferred_path.unwrap_or_default().into());
  }

  if issues.is_empty() {
    Ok(expression_paths)
  } else {
    Err(DefinitionIssues::from_vec(issues))
  }
}

fn referenced_sources(expression: &Expression) -> Vec<&str> {
  let mut sources = Vec::new();
  expression.for_each_field(&mut |source, _| {
    if !sources.contains(&source) {
      sources.push(source);
    }
  });
  sources
}
