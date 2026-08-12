use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::GraphDefinition;

mod aggregation;
mod expression;
mod projection;
mod structure;
mod topology;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionIssue {
  pub code: DefinitionIssueCode,
  pub location: String,
  pub message: String,
}

impl DefinitionIssue {
  pub(super) fn new(
    code: DefinitionIssueCode,
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
pub enum DefinitionIssueCode {
  UnsupportedVersion,
  EmptyName,
  EmptySourceKey,
  DuplicateSource,
  UnknownRoot,
  EmptyFieldName,
  DuplicateField,
  EmptyParameterName,
  DuplicateParameter,
  EmptyRelationName,
  DuplicateRelation,
  UnknownRelationSource,
  UnknownRelationTarget,
  RelationExpressionScope,
  InvalidRelationSelection,
  EmptyRelationSelectionOrder,
  RelationSelectionExpressionScope,
  InvalidExistsContext,
  UnknownExistsSource,
  InvalidExistsSource,
  ExistsExpressionScope,
  InvalidParameterShape,
  UnknownFieldSource,
  UnknownField,
  UnknownParameter,
  EmptyExpressionGroup,
  InvalidLiteral,
  IncompatibleExpressionTypes,
  InvalidExpressionType,
  InvalidFunctionArity,
  UnresolvedExpressionType,
  InvalidPredicateType,
  InvalidOrderExpression,
  EmptyOrderingName,
  DuplicateOrdering,
  EmptyOrdering,
  MultipleDefaultOrderings,
  MixedProjectionRoles,
  InvalidAggregateContext,
  InvalidDimensionExpression,
  InvalidMeasureExpression,
  NestedAggregate,
  UngroupedExpression,
  EmptyProjectionPath,
  EmptyProjectionPathSegment,
  InvalidProjectionPathSegment,
  DuplicateProjectionPath,
  ConflictingProjectionPath,
  DuplicateProjectionObjectPath,
  ProjectionObjectWithoutFields,
  ProjectionObjectInSummary,
  HiddenProjectionField,
  ProjectionExpressionScope,
  RootHasIncomingRelation,
  AmbiguousSourcePath,
  UnreachableSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DefinitionIssues(Vec<DefinitionIssue>);

impl DefinitionIssues {
  pub(crate) fn from_vec(issues: Vec<DefinitionIssue>) -> Self {
    Self(issues)
  }

  pub fn as_slice(&self) -> &[DefinitionIssue] {
    &self.0
  }

  pub fn into_vec(self) -> Vec<DefinitionIssue> {
    self.0
  }
}

impl fmt::Display for DefinitionIssues {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "query graph definition contains {} issue(s)",
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

impl Error for DefinitionIssues {}

pub(crate) fn validate(definition: &GraphDefinition) -> Result<(), DefinitionIssues> {
  structure::validate(definition)
}

pub(crate) fn infer_projection_relation_paths(
  definition: &GraphDefinition,
  source_index: &std::collections::HashMap<String, usize>,
  relation_paths: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, DefinitionIssues> {
  projection::infer_relation_paths(definition, source_index, relation_paths)
}

pub(crate) fn infer_projection_object_relation_paths(
  definition: &GraphDefinition,
  source_index: &std::collections::HashMap<String, usize>,
  relation_paths: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, DefinitionIssues> {
  projection::infer_object_relation_paths(definition, source_index, relation_paths)
}
