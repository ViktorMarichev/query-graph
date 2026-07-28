use std::collections::HashMap;

use crate::{CompiledGraph, CompiledRelationalMapping, Expression, PlanError, QueryOperation};

use super::{ParameterBinding, SqlCompileError, SqlDialect};

pub(super) struct Renderer<'a, D> {
  graph: &'a CompiledGraph,
  mapping: &'a CompiledRelationalMapping,
  operation: &'a QueryOperation,
  dialect: &'a D,
  binding_names: HashMap<String, String>,
  bindings: Vec<ParameterBinding>,
}

impl<'a, D: SqlDialect> Renderer<'a, D> {
  pub(super) fn new(
    graph: &'a CompiledGraph,
    mapping: &'a CompiledRelationalMapping,
    operation: &'a QueryOperation,
    dialect: &'a D,
  ) -> Self {
    Self {
      graph,
      mapping,
      operation,
      dialect,
      binding_names: HashMap::new(),
      bindings: Vec::new(),
    }
  }

  pub(super) fn render_source(&self, source: &str) -> Result<String, SqlCompileError> {
    let source_mapping = self
      .mapping
      .source(source)
      .ok_or_else(|| SqlCompileError::MissingSourceMapping(source.to_owned()))?;
    let table = self.dialect.render_table_name(&source_mapping.table)?;

    let alias = self.source_alias(source)?;
    Ok(self.dialect.render_table_reference(&table, &alias))
  }

  pub(super) fn render_expression(
    &mut self,
    expression: &Expression,
  ) -> Result<String, SqlCompileError> {
    match expression {
      Expression::Field { source, field } => self.render_field(source, field),
      Expression::Parameter { name } => self.render_parameter(name),
      Expression::Literal { value } => Ok(self.dialect.render_literal(value)),
      Expression::Eq { left, right } => self.render_comparison(left, "=", right, false),
      Expression::NotEq { left, right } => self.render_comparison(left, "<>", right, true),
      Expression::LessThan { left, right } => self.render_binary(left, "<", right),
      Expression::LessThanOrEqual { left, right } => self.render_binary(left, "<=", right),
      Expression::GreaterThan { left, right } => self.render_binary(left, ">", right),
      Expression::GreaterThanOrEqual { left, right } => self.render_binary(left, ">=", right),
      Expression::Like {
        expression,
        pattern,
      } => self.render_binary(expression, "LIKE", pattern),
      Expression::In { expression, values } => self.render_in(expression, values),
      Expression::And { expressions } => self.render_expression_group(expressions, "AND"),
      Expression::Or { expressions } => self.render_expression_group(expressions, "OR"),
      Expression::Not { expression } => {
        Ok(format!("(NOT {})", self.render_expression(expression)?))
      }
      Expression::IsNull { expression } => {
        Ok(format!("({} IS NULL)", self.render_expression(expression)?))
      }
      Expression::IsNotNull { expression } => Ok(format!(
        "({} IS NOT NULL)",
        self.render_expression(expression)?
      )),
      Expression::Exists { source, predicate } => self.render_exists(source, predicate.as_deref()),
      Expression::Function { name, arguments } => {
        let arguments = self.render_expressions(arguments)?;
        Ok(self.dialect.render_function(*name, &arguments))
      }
    }
  }

  pub(super) fn render_order(
    &mut self,
    order: &crate::OrderByDefinition,
  ) -> Result<String, SqlCompileError> {
    let expression = self.render_expression(&order.expression)?;
    Ok(
      self
        .dialect
        .render_order(&expression, order.direction, order.nulls),
    )
  }

  pub(super) fn into_bindings(self) -> Vec<ParameterBinding> {
    self.bindings
  }

  fn render_field(&self, source: &str, field: &str) -> Result<String, SqlCompileError> {
    let column = self
      .mapping
      .column(source, field)
      .ok_or_else(|| SqlCompileError::MissingSourceMapping(source.to_owned()))?;
    Ok(format!(
      "{}.{}",
      self.dialect.quote_identifier(&self.source_alias(source)?),
      self.dialect.quote_identifier(column)
    ))
  }

  fn render_in(
    &mut self,
    expression: &Expression,
    values: &[Expression],
  ) -> Result<String, SqlCompileError> {
    let expression = self.render_expression(expression)?;
    let values = self.render_expressions(values)?;
    Ok(format!("({expression} IN ({}))", values.join(", ")))
  }

  fn render_exists(
    &mut self,
    source: &str,
    predicate: Option<&Expression>,
  ) -> Result<String, SqlCompileError> {
    let graph = self.graph;
    let relation_indices = graph.relation_path_indices(source).ok_or_else(|| {
      SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
        message: format!("exists expression refers to missing source {source:?}"),
      })
    })?;

    if relation_indices.is_empty() {
      return Err(SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
        message: format!("exists expression source {source:?} is the graph root"),
      }));
    }

    let first = &graph.definition().relations[relation_indices[0]];
    let mut sql = format!(
      "EXISTS (\n    SELECT 1\n    FROM {}",
      self.render_source(&first.to)?
    );

    for relation_index in &relation_indices[1..] {
      let relation = &graph.definition().relations[*relation_index];
      let target = self.render_source(&relation.to)?;
      let condition = self.render_expression(&relation.on)?;
      sql.push_str(&format!("\n    INNER JOIN {target}\n      ON {condition}"));
    }

    let correlation = self.render_expression(&first.on)?;
    sql.push_str(&format!("\n    WHERE {correlation}"));

    if let Some(predicate) = predicate {
      let predicate = self.render_expression(predicate)?;
      sql.push_str(&format!("\n      AND {predicate}"));
    }

    sql.push_str("\n  )");
    Ok(sql)
  }

  fn render_parameter(&mut self, parameter: &str) -> Result<String, SqlCompileError> {
    let definition = self
      .graph
      .parameter(parameter)
      .ok_or_else(|| SqlCompileError::MissingParameter(parameter.to_owned()))?;

    if !self.operation.parameters.contains_key(parameter) {
      return Err(SqlCompileError::MissingParameter(parameter.to_owned()));
    }

    if let Some(binding_name) = self.binding_names.get(parameter) {
      return Ok(self.dialect.render_placeholder(binding_name));
    }

    let binding_name = format!("p{}", self.bindings.len());
    self
      .binding_names
      .insert(parameter.to_owned(), binding_name.clone());
    self.bindings.push(ParameterBinding {
      name: binding_name.clone(),
      parameter: parameter.to_owned(),
      scalar_type: definition.scalar_type,
    });

    Ok(self.dialect.render_placeholder(&binding_name))
  }

  fn source_alias(&self, source: &str) -> Result<String, SqlCompileError> {
    self
      .graph
      .source_index(source)
      .map(|index| format!("t{index}"))
      .ok_or_else(|| SqlCompileError::MissingSourceMapping(source.to_owned()))
  }

  fn render_binary(
    &mut self,
    left: &Expression,
    operator: &str,
    right: &Expression,
  ) -> Result<String, SqlCompileError> {
    Ok(format!(
      "({} {operator} {})",
      self.render_expression(left)?,
      self.render_expression(right)?
    ))
  }

  fn render_comparison(
    &mut self,
    left: &Expression,
    operator: &str,
    right: &Expression,
    negated_null: bool,
  ) -> Result<String, SqlCompileError> {
    if is_null_literal(right) {
      return self.render_null_comparison(left, negated_null);
    }

    if is_null_literal(left) {
      return self.render_null_comparison(right, negated_null);
    }

    self.render_binary(left, operator, right)
  }

  fn render_null_comparison(
    &mut self,
    expression: &Expression,
    negated: bool,
  ) -> Result<String, SqlCompileError> {
    let operator = if negated { "IS NOT NULL" } else { "IS NULL" };
    Ok(format!(
      "({} {operator})",
      self.render_expression(expression)?
    ))
  }

  fn render_expression_group(
    &mut self,
    expressions: &[Expression],
    operator: &str,
  ) -> Result<String, SqlCompileError> {
    let expressions = self.render_expressions(expressions)?;
    Ok(format!("({})", expressions.join(&format!(" {operator} "))))
  }

  fn render_expressions(
    &mut self,
    expressions: &[Expression],
  ) -> Result<Vec<String>, SqlCompileError> {
    expressions
      .iter()
      .map(|expression| self.render_expression(expression))
      .collect()
  }
}

fn is_null_literal(expression: &Expression) -> bool {
  matches!(
    expression,
    Expression::Literal {
      value: crate::LiteralValue::Null
    }
  )
}
