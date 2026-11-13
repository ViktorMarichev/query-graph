use crate::TraversalPathDefinition;

use super::{Renderer, SqlCompileError, SqlDialect};

// Bound CASE size, not traversal depth: Oracle permits at most 65535 CASE arguments.
const PATH_CASE_BRANCHES: usize = 32_766;

impl<D: SqlDialect> Renderer<'_, D> {
  pub(super) fn render_traversal_path(
    &mut self,
    path: &TraversalPathDefinition,
    parameters: &[String],
    start: &mut String,
    step: &mut String,
  ) -> Result<(), SqlCompileError> {
    let root = self.graph.root().key.clone();
    let offset = usize::from(path.prefix.is_some());
    for parameter in parameters {
      let count = self.path_length(parameter)?;
      if count <= offset {
        *start = format!("({start}) AND (1 = 0)");
        continue;
      }
      if let Some(prefix) = &path.prefix {
        *start = format!(
          "({start}) AND ({} = {})",
          self.render_field(&root, prefix)?,
          self.render_parameter_binding(parameter, Some(0))?
        );
      }
      let node = self.render_field(&root, &path.node)?;
      *start = format!(
        "({start}) AND ({node} = {})",
        self.render_parameter_binding(parameter, Some(offset))?
      );
      let levels = count - offset;
      if levels == 1 {
        *step = format!("({step}) AND (1 = 0)");
        continue;
      }
      let level = self.dialect.render_traversal_level()?;
      let mut alternatives = Vec::new();
      for chunk in (offset + 1..count).step_by(PATH_CASE_BRANCHES) {
        let mut cases = Vec::new();
        for index in chunk..count.min(chunk + PATH_CASE_BRANCHES) {
          cases.push(format!(
            "WHEN {} THEN {}",
            index - offset + 1,
            self.render_parameter_binding(parameter, Some(index))?
          ));
        }
        alternatives.push(format!("{node} = CASE {level} {} END", cases.join(" ")));
      }
      *step = format!(
        "({step}) AND ({level} <= {levels}) AND ({})",
        alternatives.join(" OR ")
      );
    }
    Ok(())
  }

  fn path_length(&self, parameter: &str) -> Result<usize, SqlCompileError> {
    self
      .operation
      .parameters
      .get(parameter)
      .and_then(serde_json::Value::as_array)
      .map(Vec::len)
      .ok_or_else(|| SqlCompileError::MissingParameter(parameter.to_owned()))
  }

  pub(super) fn render_path_equals(
    &self,
    source: &str,
    parameter: &str,
  ) -> Result<String, SqlCompileError> {
    let path = self
      .graph
      .source(source)
      .and_then(|source| source.traversal.as_deref())
      .and_then(|traversal| traversal.path.as_ref())
      .expect("validated path");
    let levels = self
      .path_length(parameter)?
      .saturating_sub(usize::from(path.prefix.is_some()));
    Ok(format!(
      "({} = {levels})",
      self.render_traversal_column(source, None)?
    ))
  }
}
