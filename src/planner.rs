use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt;

use crate::{CompiledGraph, ConstraintCondition, Expression, OperationIssues, QueryOperation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryPlan {
  projection_indices: Box<[usize]>,
  relation_indices: Box<[usize]>,
  constraint_indices: Box<[usize]>,
  offset: Option<u64>,
  limit: Option<u64>,
}

impl QueryPlan {
  pub(crate) fn projection_indices(&self) -> &[usize] {
    &self.projection_indices
  }

  pub(crate) fn relation_indices(&self) -> &[usize] {
    &self.relation_indices
  }

  pub(crate) fn constraint_indices(&self) -> &[usize] {
    &self.constraint_indices
  }

  pub fn offset(&self) -> Option<u64> {
    self.offset
  }

  pub fn limit(&self) -> Option<u64> {
    self.limit
  }
}

pub(crate) fn build(
  graph: &CompiledGraph,
  operation: &QueryOperation,
) -> Result<QueryPlan, PlanError> {
  let operation_plan = operation.validate(graph)?;
  let constraint_indices: Vec<_> = graph
    .definition()
    .constraints
    .iter()
    .enumerate()
    .filter_map(|(index, constraint)| {
      let active = match &constraint.when {
        ConstraintCondition::Always => true,
        ConstraintCondition::ParameterPresent { parameter } => {
          operation.parameters.contains_key(parameter)
        }
      };
      active.then_some(index)
    })
    .collect();

  let mut required_relations = HashSet::new();
  for projection_index in &operation_plan.projection_indices {
    let projection = &graph.definition().projection.fields[*projection_index];
    for relation in &projection.relations {
      let relation_index =
        graph
          .relation_index(relation)
          .ok_or_else(|| PlanError::InvalidCompiledGraph {
            message: format!("projection refers to missing relation {relation:?}"),
          })?;
      required_relations.insert(relation_index);
    }
  }

  for constraint_index in &constraint_indices {
    add_expression_relations(
      graph,
      &graph.definition().constraints[*constraint_index].predicate,
      &mut required_relations,
    )?;
  }

  for order in &graph.definition().default_order_by {
    add_expression_relations(graph, &order.expression, &mut required_relations)?;
  }

  let relation_indices = order_relations(graph, required_relations)?;

  Ok(QueryPlan {
    projection_indices: operation_plan.projection_indices.into_boxed_slice(),
    relation_indices: relation_indices.into_boxed_slice(),
    constraint_indices: constraint_indices.into_boxed_slice(),
    offset: operation.offset,
    limit: operation.limit,
  })
}

#[derive(Debug)]
pub enum PlanError {
  Operation(OperationIssues),
  InvalidCompiledGraph { message: String },
}

impl fmt::Display for PlanError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Operation(issues) => issues.fmt(formatter),
      Self::InvalidCompiledGraph { message } => {
        write!(formatter, "invalid compiled graph: {message}")
      }
    }
  }
}

impl Error for PlanError {}

impl From<OperationIssues> for PlanError {
  fn from(issues: OperationIssues) -> Self {
    Self::Operation(issues)
  }
}

fn add_expression_relations(
  graph: &CompiledGraph,
  expression: &Expression,
  relations: &mut HashSet<usize>,
) -> Result<(), PlanError> {
  let mut sources = HashSet::new();
  expression.for_each_field(&mut |source, _| {
    sources.insert(source);
  });

  for source in sources {
    let path =
      graph
        .relation_path_indices(source)
        .ok_or_else(|| PlanError::InvalidCompiledGraph {
          message: format!("expression refers to missing source {source:?}"),
        })?;
    relations.extend(path);
  }

  Ok(())
}

fn order_relations(
  graph: &CompiledGraph,
  mut required: HashSet<usize>,
) -> Result<Vec<usize>, PlanError> {
  let mut relations = Vec::with_capacity(required.len());
  let mut queue = VecDeque::from([graph.root().key.clone()]);

  while let Some(source) = queue.pop_front() {
    let Some(outgoing) = graph.outgoing_relations(&source) else {
      continue;
    };

    for relation in outgoing {
      let relation_index =
        graph
          .relation_index(&relation.name)
          .ok_or_else(|| PlanError::InvalidCompiledGraph {
            message: format!("relation {:?} is not indexed", relation.name),
          })?;

      if required.remove(&relation_index) {
        relations.push(relation_index);
        queue.push_back(relation.to.clone());
      }
    }
  }

  if required.is_empty() {
    Ok(relations)
  } else {
    Err(PlanError::InvalidCompiledGraph {
      message: format!("relations {required:?} cannot be ordered from the root"),
    })
  }
}
