use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
  scalar::is_decimal_text, CompiledGraph, ParameterCardinality, ProjectionPath, ScalarType,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOperation {
  #[serde(default)]
  pub select: Option<Vec<String>>,
  #[serde(default)]
  pub parameters: HashMap<String, Value>,
  #[serde(default)]
  pub offset: Option<u64>,
  #[serde(default)]
  pub limit: Option<u64>,
}

impl QueryOperation {
  pub(crate) fn validate(
    &self,
    graph: &CompiledGraph,
  ) -> Result<ValidatedQueryOperation, OperationIssues> {
    let mut issues = Vec::new();
    let mut projection_indices = Vec::new();
    let mut selected_paths = HashSet::new();

    let selected_fields: Vec<String> = match &self.select {
      Some(fields) => fields.clone(),
      None => graph
        .definition()
        .projection
        .fields
        .iter()
        .filter(|field| field.selectable && field.selected_by_default)
        .map(|field| field.path.join("."))
        .collect(),
    };

    if selected_fields.is_empty() {
      issues.push(OperationIssue::new(
        OperationIssueCode::EmptySelection,
        "select",
        "query operation must select at least one field",
      ));
    }

    for (index, path) in selected_fields.iter().enumerate() {
      let projection_path = ProjectionPath::parse(path);

      if !selected_paths.insert(projection_path.clone()) {
        issues.push(OperationIssue::new(
          OperationIssueCode::DuplicateSelection,
          format!("select[{index}]"),
          format!("projection field {path:?} is selected more than once"),
        ));
        continue;
      }

      let Some(projection_index) = graph.projection_index(&projection_path) else {
        issues.push(OperationIssue::new(
          OperationIssueCode::UnknownSelection,
          format!("select[{index}]"),
          format!("projection field {path:?} is not defined"),
        ));
        continue;
      };

      let field = &graph.definition().projection.fields[projection_index];
      if !field.selectable {
        issues.push(OperationIssue::new(
          OperationIssueCode::NonSelectableField,
          format!("select[{index}]"),
          format!("projection field {path:?} cannot be selected"),
        ));
        continue;
      }

      projection_indices.push(projection_index);
    }

    for parameter in self.parameters.keys() {
      if graph.parameter(parameter).is_none() {
        issues.push(OperationIssue::new(
          OperationIssueCode::UnknownParameter,
          format!("parameters.{parameter}"),
          format!("parameter {parameter:?} is not defined by the graph"),
        ));
      }
    }

    for parameter in &graph.definition().parameters {
      match self.parameters.get(&parameter.name) {
        Some(value) => {
          if !is_valid_parameter_value(value, parameter.scalar_type, parameter.cardinality) {
            issues.push(OperationIssue::new(
              OperationIssueCode::InvalidParameterType,
              format!("parameters.{}", parameter.name),
              format!(
                "expected {:?} with {:?} cardinality",
                parameter.scalar_type, parameter.cardinality
              ),
            ));
          }
        }
        None if parameter.required => {
          issues.push(OperationIssue::new(
            OperationIssueCode::MissingParameter,
            format!("parameters.{}", parameter.name),
            format!("required parameter {:?} is missing", parameter.name),
          ));
        }
        None => {}
      }
    }

    if self.limit == Some(0) {
      issues.push(OperationIssue::new(
        OperationIssueCode::InvalidPagination,
        "limit",
        "limit must be greater than zero",
      ));
    }

    if issues.is_empty() {
      Ok(ValidatedQueryOperation { projection_indices })
    } else {
      Err(OperationIssues(issues))
    }
  }
}

#[derive(Debug)]
pub(crate) struct ValidatedQueryOperation {
  pub projection_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationIssue {
  pub code: OperationIssueCode,
  pub location: String,
  pub message: String,
}

impl OperationIssue {
  fn new(
    code: OperationIssueCode,
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
pub enum OperationIssueCode {
  EmptySelection,
  UnknownSelection,
  NonSelectableField,
  DuplicateSelection,
  MissingParameter,
  UnknownParameter,
  InvalidParameterType,
  InvalidPagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationIssues(Vec<OperationIssue>);

impl OperationIssues {
  pub fn as_slice(&self) -> &[OperationIssue] {
    &self.0
  }

  pub fn into_vec(self) -> Vec<OperationIssue> {
    self.0
  }
}

impl fmt::Display for OperationIssues {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "query operation contains {} issue(s)",
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

impl Error for OperationIssues {}

fn is_valid_parameter_value(
  value: &Value,
  scalar_type: ScalarType,
  cardinality: ParameterCardinality,
) -> bool {
  match cardinality {
    ParameterCardinality::One => is_valid_scalar(value, scalar_type),
    ParameterCardinality::Many => value.as_array().is_some_and(|values| {
      values
        .iter()
        .all(|value| is_valid_scalar(value, scalar_type))
    }),
  }
}

fn is_valid_scalar(value: &Value, scalar_type: ScalarType) -> bool {
  match scalar_type {
    ScalarType::Boolean => value.is_boolean(),
    ScalarType::Int32 => value
      .as_i64()
      .is_some_and(|value| i32::try_from(value).is_ok()),
    ScalarType::Int64 => {
      value.as_i64().is_some()
        || value
          .as_u64()
          .is_some_and(|value| i64::try_from(value).is_ok())
        || value.as_f64().is_some_and(is_safe_javascript_integer)
        || value
          .as_str()
          .is_some_and(|value| value.parse::<i64>().is_ok())
    }
    ScalarType::Float64 => value.as_f64().is_some(),
    ScalarType::Decimal => value.is_number() || value.as_str().is_some_and(is_decimal_text),
    ScalarType::String | ScalarType::Date | ScalarType::DateTime | ScalarType::Binary => {
      value.is_string()
    }
    ScalarType::Json => true,
  }
}

fn is_safe_javascript_integer(value: f64) -> bool {
  const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

  value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER
}
