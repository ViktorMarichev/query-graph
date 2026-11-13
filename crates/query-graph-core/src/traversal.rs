use serde::{Deserialize, Serialize};

use crate::{Expression, GraphDefinition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraversalDefinition {
  pub scope: String,
  pub input: GraphDefinition,
  pub key: String,
  pub start: Expression,
  pub step: Expression,
  pub cycles: CyclePolicy,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub path: Option<TraversalPathDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraversalPathDefinition {
  pub node: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prefix: Option<String>,
}

// The two wire forms share the same analysis and rendering contract.
#[derive(Clone, Copy)]
pub(crate) struct TraversalRef<'a> {
  pub scope: &'a str,
  pub input: &'a GraphDefinition,
  pub key: &'a str,
  pub start: &'a Expression,
  pub step: &'a Expression,
}

impl<'a> From<&'a TraversalDefinition> for TraversalRef<'a> {
  fn from(value: &'a TraversalDefinition) -> Self {
    Self {
      scope: &value.scope,
      input: &value.input,
      key: &value.key,
      start: &value.start,
      step: &value.step,
    }
  }
}

impl<'a> From<&'a ReachableExpression> for TraversalRef<'a> {
  fn from(value: &'a ReachableExpression) -> Self {
    Self {
      scope: &value.scope,
      input: &value.input,
      key: &value.key,
      start: &value.start,
      step: &value.step,
    }
  }
}

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
