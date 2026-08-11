use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
  scalar::is_decimal_text, CompiledGraph, ParameterDefinition, ParameterShape, ProjectionPath,
  ScalarType,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryOperation {
  #[serde(default)]
  pub select: Option<Vec<String>>,
  #[serde(default)]
  pub ordering: Option<String>,
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
        .filter(|field| field.selected_by_default)
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

      projection_indices.push(projection_index);
    }

    let ordering_index = match self.ordering.as_deref() {
      Some(name) => match graph.ordering_index(name) {
        Some(index) => Some(index),
        None => {
          issues.push(OperationIssue::new(
            OperationIssueCode::UnknownOrdering,
            "ordering",
            format!("ordering {name:?} is not defined by the graph"),
          ));
          None
        }
      },
      None => graph.default_ordering_index(),
    };

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
          validate_parameter_value(parameter, value, &mut issues);
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
      Ok(ValidatedQueryOperation {
        projection_indices,
        ordering_index,
      })
    } else {
      Err(OperationIssues(issues))
    }
  }

  pub(crate) fn validate_plan_parameters<'a>(
    &self,
    parameters: impl IntoIterator<Item = &'a str>,
  ) -> Result<(), OperationIssues> {
    let mut missing: Vec<_> = parameters
      .into_iter()
      .filter(|parameter| !self.parameters.contains_key(*parameter))
      .collect();
    missing.sort_unstable();
    missing.dedup();

    if missing.is_empty() {
      return Ok(());
    }

    Err(OperationIssues(
      missing
        .into_iter()
        .map(|parameter| {
          OperationIssue::new(
            OperationIssueCode::MissingParameter,
            format!("parameters.{parameter}"),
            format!("parameter {parameter:?} is required by the selected query plan"),
          )
        })
        .collect(),
    ))
  }
}

#[derive(Debug)]
pub(crate) struct ValidatedQueryOperation {
  pub projection_indices: Vec<usize>,
  pub ordering_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationIssue {
  pub code: OperationIssueCode,
  pub location: String,
  pub message: String,
}

impl OperationIssue {
  pub(crate) fn new(
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
  DuplicateSelection,
  UnknownOrdering,
  MissingParameter,
  UnknownParameter,
  InvalidParameterType,
  InvalidPagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationIssues(Vec<OperationIssue>);

impl OperationIssues {
  pub(crate) fn from_vec(issues: Vec<OperationIssue>) -> Self {
    Self(issues)
  }

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

fn validate_parameter_value(
  parameter: &ParameterDefinition,
  value: &Value,
  issues: &mut Vec<OperationIssue>,
) {
  let location = format!("parameters.{}", parameter.name);
  match parameter.shape {
    ParameterShape::Scalar => {
      if !is_valid_scalar(value, parameter.scalar_type) {
        issues.push(OperationIssue::new(
          OperationIssueCode::InvalidParameterType,
          location,
          format!("expected {:?}", parameter.scalar_type),
        ));
      }
    }
    ParameterShape::List => {
      let Some(values) = value.as_array() else {
        issues.push(OperationIssue::new(
          OperationIssueCode::InvalidParameterType,
          location,
          format!("expected list of {:?}", parameter.scalar_type),
        ));
        return;
      };

      for (index, value) in values.iter().enumerate() {
        if !is_valid_scalar(value, parameter.scalar_type) {
          issues.push(OperationIssue::new(
            OperationIssueCode::InvalidParameterType,
            format!("{location}[{index}]"),
            format!("expected {:?}", parameter.scalar_type),
          ));
        }
      }
    }
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

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::is_valid_scalar;
  use crate::ScalarType;

  #[test]
  fn accepts_the_documented_scalar_wire_values() {
    let examples = [
      (json!(true), ScalarType::Boolean),
      (json!(2_147_483_647), ScalarType::Int32),
      (json!(9_007_199_254_740_991_i64), ScalarType::Int64),
      (json!("9223372036854775807"), ScalarType::Int64),
      (json!(1.5), ScalarType::Float64),
      (json!(1.5), ScalarType::Decimal),
      (json!("1234567890.123456789"), ScalarType::Decimal),
      (json!("value"), ScalarType::String),
      (json!("2026-07-28"), ScalarType::Date),
      (json!("2026-07-28T10:15:30+06:00"), ScalarType::DateTime),
      (json!("base64-or-driver-token"), ScalarType::Binary),
      (json!({"nested": [1, true]}), ScalarType::Json),
    ];

    for (value, scalar_type) in examples {
      assert!(
        is_valid_scalar(&value, scalar_type),
        "{value} should satisfy {scalar_type:?}"
      );
    }
  }

  #[test]
  fn rejects_values_outside_the_scalar_wire_contract() {
    let examples = [
      (json!(1), ScalarType::Boolean),
      (json!(2_147_483_648_i64), ScalarType::Int32),
      (json!(1.5), ScalarType::Int64),
      (json!("1e3"), ScalarType::Decimal),
      (json!(false), ScalarType::String),
      (json!({"year": 2026}), ScalarType::Date),
      (json!([1, 2, 3]), ScalarType::Binary),
    ];

    for (value, scalar_type) in examples {
      assert!(
        !is_valid_scalar(&value, scalar_type),
        "{value} should not satisfy {scalar_type:?}"
      );
    }
  }
}
