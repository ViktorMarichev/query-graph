use serde::{Deserialize, Serialize};

use crate::{Expression, GraphDefinition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReachableExpression {
  pub scope: String,
  pub input: GraphDefinition,
  pub key: String,
  pub start: Expression,
  pub step: Expression,
  #[serde(rename = "match")]
  pub predicate: Expression,
  pub cycles: CyclePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CyclePolicy {
  Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraversalSide {
  Current,
  Next,
}
