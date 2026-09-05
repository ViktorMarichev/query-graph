use crate::{
  analysis::{DefinitionIndex, GraphTopology},
  GraphDefinition,
};

use super::{DefinitionIssue, DefinitionIssueCode, DefinitionIssues};

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
  Unvisited,
  Visiting,
  Done,
}

pub(super) fn validate(
  definition: &GraphDefinition,
  index: &DefinitionIndex,
  topology: &GraphTopology,
) -> Result<(), DefinitionIssues> {
  // EXISTS renders every relation predicate on its correlation path again.
  // These dependencies can cycle even though the source topology is a tree.
  let dependencies: Vec<Vec<usize>> = definition
    .relations
    .iter()
    .map(|relation| {
      let mut dependencies = Vec::new();
      relation.on.for_each_exists_source(&mut |source, from| {
        let from = from.unwrap_or(&definition.root);
        if let (Some(from), Some(source)) = (index.source(from), index.source(source)) {
          if let Some(path) = topology.relation_path_between(from, source) {
            dependencies.extend_from_slice(path);
          }
        }
      });
      dependencies.sort_unstable();
      dependencies.dedup();
      dependencies
    })
    .collect();

  let mut states = vec![VisitState::Unvisited; dependencies.len()];
  let mut cycle_targets = vec![None; dependencies.len()];
  for start in 0..dependencies.len() {
    if states[start] != VisitState::Unvisited {
      continue;
    }
    states[start] = VisitState::Visiting;
    let mut stack = vec![(start, 0)];
    while let Some((relation, next)) = stack.last_mut() {
      let Some(&dependency) = dependencies[*relation].get(*next) else {
        states[*relation] = VisitState::Done;
        stack.pop();
        continue;
      };
      *next += 1;
      match states[dependency] {
        VisitState::Unvisited => {
          states[dependency] = VisitState::Visiting;
          stack.push((dependency, 0));
        }
        VisitState::Visiting => {
          cycle_targets[*relation].get_or_insert(dependency);
        }
        VisitState::Done => {}
      }
    }
  }

  let issues: Vec<_> = cycle_targets
    .into_iter()
    .enumerate()
    .filter_map(|(relation, target)| {
      target.map(|target| {
        DefinitionIssue::new(
          DefinitionIssueCode::InvalidExistsContext,
          format!("relations[{relation}].on"),
          format!(
            "exists dependency from relation {:?} to {:?} forms a cycle",
            definition.relations[relation].name, definition.relations[target].name,
          ),
        )
      })
    })
    .collect();
  if issues.is_empty() {
    Ok(())
  } else {
    Err(DefinitionIssues::from_vec(issues))
  }
}
