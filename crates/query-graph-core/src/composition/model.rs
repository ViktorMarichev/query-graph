use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{MappedQueryGraph, OperationIssues, RelationCardinality, SqlCompileError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRelationDefinition {
  pub name: String,
  pub from: String,
  pub to: String,
  pub parameter: String,
  pub cardinality: RelationCardinality,
  #[serde(default)]
  pub parameters: HashMap<String, Value>,
  #[serde(default)]
  pub ordering: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionIssue {
  pub code: CompositionIssueCode,
  pub location: String,
  pub message: String,
}

impl CompositionIssue {
  pub(super) fn new(
    code: CompositionIssueCode,
    location: impl Into<String>,
    message: impl Into<String>,
  ) -> Self {
    Self {
      code,
      location: location.into(),
      message: message.into(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompositionIssueCode {
  EmptyRelationName,
  InvalidRelationName,
  DuplicateRelationName,
  ConflictingProjectionPath,
  UnknownParentKey,
  UnknownChildKey,
  UnknownKeyParameter,
  KeyParameterNotList,
  IncompatibleKeyTypes,
  KeyParameterIsStatic,
  UnknownStaticParameter,
  MissingStaticParameter,
  InvalidStaticParameterType,
  UnknownChildOrdering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompositionIssues(Vec<CompositionIssue>);

impl CompositionIssues {
  pub(super) fn from_vec(issues: Vec<CompositionIssue>) -> Self {
    Self(issues)
  }

  pub fn as_slice(&self) -> &[CompositionIssue] {
    &self.0
  }

  pub fn into_vec(self) -> Vec<CompositionIssue> {
    self.0
  }
}

impl fmt::Display for CompositionIssues {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "query graph composition contains {} issue(s)",
      self.0.len()
    )?;

    for issue in &self.0 {
      write!(
        formatter,
        "\n- {:?} at {}: {}",
        issue.code, issue.location, issue.message
      )?;
    }

    Ok(())
  }
}

impl Error for CompositionIssues {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanMetadata {
  pub name: String,
  pub parent_key: String,
  pub child_key: String,
  pub key_parameter: String,
  pub parameters: HashMap<String, Value>,
  pub cardinality: RelationCardinality,
  pub parent_key_injected: bool,
  pub child_key_injected: bool,
}

#[derive(Debug)]
pub enum ComposedCompileError {
  Operation(OperationIssues),
  Sql(SqlCompileError),
  UnknownSelectedBatchRelation(String),
}

impl fmt::Display for ComposedCompileError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Operation(error) => error.fmt(formatter),
      Self::Sql(error) => error.fmt(formatter),
      Self::UnknownSelectedBatchRelation(name) => {
        write!(
          formatter,
          "batch relation {name:?} is not selected by the query plan"
        )
      }
    }
  }
}

impl Error for ComposedCompileError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      Self::Operation(error) => Some(error),
      Self::Sql(error) => Some(error),
      Self::UnknownSelectedBatchRelation(_) => None,
    }
  }
}

impl From<OperationIssues> for ComposedCompileError {
  fn from(error: OperationIssues) -> Self {
    Self::Operation(error)
  }
}

impl From<SqlCompileError> for ComposedCompileError {
  fn from(error: SqlCompileError) -> Self {
    Self::Sql(error)
  }
}

#[derive(Debug, Clone)]
pub(super) struct BatchRelation {
  pub(super) definition: BatchRelationDefinition,
  pub(super) graph: MappedQueryGraph,
}
