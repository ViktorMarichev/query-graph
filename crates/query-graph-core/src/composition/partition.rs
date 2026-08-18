use std::collections::{HashMap, HashSet};

use crate::{
  MappedQueryGraph, OperationIssue, OperationIssueCode, OperationIssues, QueryOperation,
};

use super::model::{BatchPlanMetadata, BatchRelation};

#[derive(Debug)]
pub(super) struct PartitionedBatch {
  pub(super) relation_index: usize,
  pub(super) metadata: BatchPlanMetadata,
  pub(super) operation: QueryOperation,
}

#[derive(Debug)]
pub(super) struct PartitionedOperation {
  pub(super) root: QueryOperation,
  pub(super) batches: Vec<PartitionedBatch>,
}

pub(super) fn partition_operation(
  root: &MappedQueryGraph,
  relations: &[BatchRelation],
  operation: &QueryOperation,
) -> Result<PartitionedOperation, OperationIssues> {
  let Some(selected_paths) = operation.select.as_ref() else {
    return Ok(PartitionedOperation {
      root: operation.clone(),
      batches: Vec::new(),
    });
  };

  let mut issues = Vec::new();
  let mut seen = HashSet::new();
  let mut root_select = Vec::new();
  let mut requested_root_paths = HashSet::new();
  let mut child_select: HashMap<usize, Vec<String>> = HashMap::new();

  for (index, path) in selected_paths.iter().enumerate() {
    if !seen.insert(path.as_str()) {
      issues.push(OperationIssue::new(
        OperationIssueCode::DuplicateSelection,
        format!("select[{index}]"),
        format!("projection field {path:?} is selected more than once"),
      ));
      continue;
    }

    if root.graph().projection(path).is_some() {
      root_select.push(path.clone());
      requested_root_paths.insert(path.as_str());
      continue;
    }

    let relation = relations
      .iter()
      .enumerate()
      .find_map(|(relation_index, relation)| {
        let prefix = format!("{}.", relation.definition.name);
        let child_path = path.strip_prefix(&prefix)?;
        relation
          .graph
          .graph()
          .projection(child_path)
          .map(|_| (relation_index, child_path))
      });

    if let Some((relation_index, child_path)) = relation {
      child_select
        .entry(relation_index)
        .or_default()
        .push(child_path.to_owned());
    } else {
      issues.push(OperationIssue::new(
        OperationIssueCode::UnknownSelection,
        format!("select[{index}]"),
        format!("projection field {path:?} is not defined"),
      ));
    }
  }

  if !issues.is_empty() {
    return Err(OperationIssues::from_vec(issues));
  }

  let mut batches = Vec::new();
  for (relation_index, relation) in relations.iter().enumerate() {
    let Some(mut selection) = child_select.remove(&relation_index) else {
      continue;
    };
    let parent_key_injected = !requested_root_paths.contains(relation.definition.from.as_str());
    let child_key_injected = !selection.contains(&relation.definition.to);

    if !root_select.contains(&relation.definition.from) {
      root_select.push(relation.definition.from.clone());
    }
    if child_key_injected {
      selection.push(relation.definition.to.clone());
    }

    batches.push(PartitionedBatch {
      relation_index,
      metadata: BatchPlanMetadata {
        name: relation.definition.name.clone(),
        parent_key: relation.definition.from.clone(),
        child_key: relation.definition.to.clone(),
        key_parameter: relation.definition.parameter.clone(),
        parameters: relation.definition.parameters.clone(),
        cardinality: relation.definition.cardinality,
        parent_key_injected,
        child_key_injected,
      },
      operation: QueryOperation {
        select: Some(selection),
        ordering: relation.definition.ordering.clone(),
        parameters: relation.definition.parameters.clone(),
        ..QueryOperation::default()
      },
    });
  }

  Ok(PartitionedOperation {
    root: QueryOperation {
      select: Some(root_select),
      ordering: operation.ordering.clone(),
      parameters: operation.parameters.clone(),
      offset: operation.offset,
      limit: operation.limit,
    },
    batches,
  })
}
