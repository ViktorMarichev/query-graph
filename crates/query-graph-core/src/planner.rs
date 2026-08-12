use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt;

use crate::{
  compiled_graph::ConstraintPhase, CompiledGraph, ConstraintCondition, Expression, OperationIssues,
  OrderByDefinition, QueryOperation, RelationCardinality,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryPlan {
  projection_indices: Box<[usize]>,
  projection_object_indices: Box<[usize]>,
  relation_indices: Box<[usize]>,
  pre_aggregation_constraint_indices: Box<[usize]>,
  post_aggregation_constraint_indices: Box<[usize]>,
  ordering_index: Option<usize>,
  offset: Option<u64>,
  limit: Option<u64>,
}

impl QueryPlan {
  pub(crate) fn projection_indices(&self) -> &[usize] {
    &self.projection_indices
  }

  pub(crate) fn projection_object_indices(&self) -> &[usize] {
    &self.projection_object_indices
  }

  pub(crate) fn relation_indices(&self) -> &[usize] {
    &self.relation_indices
  }

  pub(crate) fn pre_aggregation_constraint_indices(&self) -> &[usize] {
    &self.pre_aggregation_constraint_indices
  }

  pub(crate) fn post_aggregation_constraint_indices(&self) -> &[usize] {
    &self.post_aggregation_constraint_indices
  }

  pub(crate) fn order_by<'a>(&self, graph: &'a CompiledGraph) -> &'a [OrderByDefinition] {
    self
      .ordering_index
      .map(|index| graph.ordering_at(index).order_by.as_slice())
      .unwrap_or(&[])
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
  let projection_object_indices =
    graph.selected_projection_object_indices(&operation_plan.projection_indices);
  let order_by = operation_plan
    .ordering_index
    .map(|index| graph.ordering_at(index).order_by.as_slice())
    .unwrap_or(&[]);
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
    required_relations.extend(
      graph
        .projection_relation_path_indices(*projection_index)
        .iter()
        .copied(),
    );
  }
  for object_index in &projection_object_indices {
    required_relations.extend(
      graph
        .projection_object_relation_path_indices(*object_index)
        .iter()
        .copied(),
    );
  }

  for projection_index in graph.dimension_projection_indices() {
    required_relations.extend(
      graph
        .projection_relation_path_indices(*projection_index)
        .iter()
        .copied(),
    );
  }

  for constraint_index in &constraint_indices {
    add_expression_relations(
      graph,
      &graph.definition().constraints[*constraint_index].predicate,
      &mut required_relations,
    )?;
  }

  for order in order_by {
    add_expression_relations(graph, &order.expression, &mut required_relations)?;
  }

  let relation_indices = order_relations(graph, required_relations)?;
  let mut required_parameters = HashSet::new();
  for projection_index in &operation_plan.projection_indices {
    add_expression_parameters(
      graph,
      &graph.definition().projection.fields[*projection_index].expression,
      &mut required_parameters,
    )?;
  }
  for object_index in &projection_object_indices {
    add_expression_parameters(
      graph,
      &graph.definition().projection.objects[*object_index].presence,
      &mut required_parameters,
    )?;
  }
  for projection_index in graph.dimension_projection_indices() {
    add_expression_parameters(
      graph,
      &graph.projection_at(*projection_index).expression,
      &mut required_parameters,
    )?;
  }
  for constraint_index in &constraint_indices {
    add_expression_parameters(
      graph,
      &graph.definition().constraints[*constraint_index].predicate,
      &mut required_parameters,
    )?;
  }
  for order in order_by {
    add_expression_parameters(graph, &order.expression, &mut required_parameters)?;
  }
  for relation_index in &relation_indices {
    let relation = &graph.definition().relations[*relation_index];
    add_expression_parameters(graph, &relation.on, &mut required_parameters)?;
    if let Some(selection) = &relation.selection {
      for order in selection.order_by() {
        add_expression_parameters(graph, &order.expression, &mut required_parameters)?;
      }
    }
  }
  operation.validate_plan_parameters(required_parameters)?;

  if graph.is_summary() {
    validate_summary_relation_shape(graph, &relation_indices)?;
  }

  if !graph.is_summary() && (operation.offset.is_some() || operation.limit.is_some()) {
    if let Some(relation) = relation_indices
      .iter()
      .map(|index| &graph.definition().relations[*index])
      .find(|relation| relation.cardinality == RelationCardinality::Many)
    {
      return Err(PlanError::PaginationThroughManyRelation {
        relation: relation.name.clone(),
      });
    }
  }

  let (pre_aggregation_constraint_indices, post_aggregation_constraint_indices): (Vec<_>, Vec<_>) =
    constraint_indices
      .into_iter()
      .partition(|index| graph.constraint_phase(*index) == ConstraintPhase::BeforeAggregation);

  Ok(QueryPlan {
    projection_indices: operation_plan.projection_indices.into_boxed_slice(),
    projection_object_indices: projection_object_indices.into_boxed_slice(),
    relation_indices: relation_indices.into_boxed_slice(),
    pre_aggregation_constraint_indices: pre_aggregation_constraint_indices.into_boxed_slice(),
    post_aggregation_constraint_indices: post_aggregation_constraint_indices.into_boxed_slice(),
    ordering_index: operation_plan.ordering_index,
    offset: operation.offset,
    limit: operation.limit,
  })
}

#[derive(Debug)]
pub enum PlanError {
  Operation(OperationIssues),
  InvalidCompiledGraph {
    message: String,
  },
  PaginationThroughManyRelation {
    relation: String,
  },
  AggregationAcrossManyBranches {
    left_relation: String,
    right_relation: String,
  },
}

impl fmt::Display for PlanError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Operation(issues) => issues.fmt(formatter),
      Self::InvalidCompiledGraph { message } => {
        write!(formatter, "invalid compiled graph: {message}")
      }
      Self::PaginationThroughManyRelation { relation } => write!(
        formatter,
        "pagination through many relation {relation:?} requires a split query plan"
      ),
      Self::AggregationAcrossManyBranches {
        left_relation,
        right_relation,
      } => write!(
        formatter,
        "aggregation across independent many relations {left_relation:?} and {right_relation:?} requires an aggregate subquery plan"
      ),
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
  expression.for_each_outer_source(&mut |source| {
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

fn add_expression_parameters<'a>(
  graph: &'a CompiledGraph,
  expression: &'a Expression,
  parameters: &mut HashSet<&'a str>,
) -> Result<(), PlanError> {
  expression.for_each_parameter(&mut |parameter| {
    parameters.insert(parameter);
  });

  let mut exists_sources = Vec::new();
  expression.for_each_exists_source(&mut |source, from| {
    exists_sources.push((source, from));
  });

  for (source, from) in exists_sources {
    let from = from.unwrap_or(graph.root().key.as_str());
    let relation_path = graph
      .relation_path_indices_between(from, source)
      .ok_or_else(|| PlanError::InvalidCompiledGraph {
        message: format!("exists expression source {source:?} is not reachable from {from:?}"),
      })?;

    for relation_index in relation_path {
      let relation = &graph.definition().relations[*relation_index];
      relation.on.for_each_parameter(&mut |parameter| {
        parameters.insert(parameter);
      });
      if let Some(selection) = &relation.selection {
        for order in selection.order_by() {
          order.expression.for_each_parameter(&mut |parameter| {
            parameters.insert(parameter);
          });
        }
      }
    }
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

fn validate_summary_relation_shape(
  graph: &CompiledGraph,
  relation_indices: &[usize],
) -> Result<(), PlanError> {
  let many_relations: Vec<_> = relation_indices
    .iter()
    .map(|index| &graph.definition().relations[*index])
    .filter(|relation| relation.cardinality == RelationCardinality::Many)
    .collect();

  for (index, left) in many_relations.iter().enumerate() {
    let left_path =
      graph
        .relation_path_indices(&left.to)
        .ok_or_else(|| PlanError::InvalidCompiledGraph {
          message: format!("relation target {:?} has no path from the root", left.to),
        })?;

    for right in &many_relations[index + 1..] {
      let right_path =
        graph
          .relation_path_indices(&right.to)
          .ok_or_else(|| PlanError::InvalidCompiledGraph {
            message: format!("relation target {:?} has no path from the root", right.to),
          })?;

      if !left_path.starts_with(right_path) && !right_path.starts_with(left_path) {
        return Err(PlanError::AggregationAcrossManyBranches {
          left_relation: left.name.clone(),
          right_relation: right.name.clone(),
        });
      }
    }
  }

  Ok(())
}
