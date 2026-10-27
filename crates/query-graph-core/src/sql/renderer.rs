use std::collections::HashMap;

use crate::{
  AggregateFunction, CompiledGraph, CompiledRelationalMapping, Expression, PlanError,
  QueryOperation, RelationDefinition, ScalarType,
};

use super::{BindingPathSegment, ParameterBinding, SqlCompileError, SqlDialect};

const MAX_GROUP_EXPANSIONS: usize = 10_000;
const MAX_GROUP_VALUE_EXPANSIONS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BindingKey {
  parameter: String,
  address: BindingAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BindingAddress {
  Scalar,
  Index(usize),
  Path(Vec<BindingPathSegment>),
}

#[derive(Clone)]
struct GroupScope {
  scope: String,
  parameter: String,
  index: usize,
}

pub(super) struct Renderer<'a, D> {
  graph: &'a CompiledGraph,
  mapping: &'a CompiledRelationalMapping,
  operation: &'a QueryOperation,
  dialect: &'a D,
  binding_names: HashMap<BindingKey, String>,
  bindings: Vec<ParameterBinding>,
  group_scopes: Vec<GroupScope>,
  group_expansions: usize,
  group_value_expansions: usize,
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
      group_scopes: Vec::new(),
      group_expansions: 0,
      group_value_expansions: 0,
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

  pub(super) fn render_value(
    &mut self,
    expression: &Expression,
  ) -> Result<String, SqlCompileError> {
    match expression {
      Expression::Field { source, field } => self.render_field(source, field),
      Expression::Parameter { name } => self.render_parameter(name),
      Expression::GroupKey { scope } => self.render_group_key(scope),
      Expression::Literal { value } => Ok(self.dialect.render_literal(value)),
      Expression::Function { name, arguments } => {
        let arguments = self.render_values(arguments)?;
        Ok(self.dialect.render_function(*name, &arguments))
      }
      Expression::Aggregate {
        function,
        expression,
      } => self.render_aggregate(*function, expression.as_deref()),
      _ => {
        // Render once before reusing the predicate text in a three-valued CASE.
        let predicate = self.render_predicate(expression)?;
        Ok(self.dialect.render_predicate_value(&predicate))
      }
    }
  }

  pub(super) fn render_predicate(
    &mut self,
    expression: &Expression,
  ) -> Result<String, SqlCompileError> {
    match expression {
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
      Expression::InParameter {
        expression,
        parameter,
      } => self.render_in_parameter(expression, parameter),
      Expression::AllGroups {
        parameter,
        scope,
        predicate,
      } => self.render_all_groups(parameter, scope, predicate),
      Expression::InGroupValues { expression, scope } => {
        self.render_in_group_values(expression, scope)
      }
      Expression::And { expressions } => self.render_predicate_group(expressions, "AND"),
      Expression::Or { expressions } => self.render_predicate_group(expressions, "OR"),
      Expression::Not { expression } => Ok(format!("(NOT {})", self.render_predicate(expression)?)),
      Expression::IsNull { expression } => {
        Ok(format!("({} IS NULL)", self.render_value(expression)?))
      }
      Expression::IsNotNull { expression } => {
        Ok(format!("({} IS NOT NULL)", self.render_value(expression)?))
      }
      Expression::Exists {
        source,
        from,
        predicate,
      } => self.render_exists(source, from.as_deref(), predicate.as_deref()),
      Expression::Field { .. }
      | Expression::Parameter { .. }
      | Expression::GroupKey { .. }
      | Expression::Literal { .. }
      | Expression::Function { .. }
      | Expression::Aggregate { .. } => {
        let value = self.render_value(expression)?;
        Ok(self.dialect.render_value_predicate(&value))
      }
    }
  }

  pub(super) fn render_order(
    &mut self,
    order: &crate::OrderByDefinition,
  ) -> Result<String, SqlCompileError> {
    let expression = self.render_value(&order.expression)?;
    Ok(
      self
        .dialect
        .render_order(&expression, order.direction, order.nulls),
    )
  }

  pub(super) fn render_selected_relation_source(
    &mut self,
    relation: &RelationDefinition,
  ) -> Result<String, SqlCompileError> {
    let selection = relation.selection.as_ref().ok_or_else(|| {
      SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
        message: format!("relation {:?} has no selection strategy", relation.name),
      })
    })?;
    let order_by: Result<Vec<_>, _> = selection
      .order_by()
      .iter()
      .map(|order| self.render_order(order))
      .collect();
    let order_by = order_by?;
    if order_by.is_empty() {
      return Err(SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
        message: format!("relation {:?} has an empty firstBy order", relation.name),
      }));
    }

    let alias = self.source_alias(&relation.to)?;
    let projection = format!("{}.*", self.dialect.quote_identifier(&alias));
    let source = self.render_source(&relation.to)?;
    let predicate = self.render_predicate(&relation.on)?;
    let query = self
      .dialect
      .render_first_by_query(&projection, &source, &predicate, &order_by)?;
    let indented = query
      .lines()
      .map(|line| format!("    {line}"))
      .collect::<Vec<_>>()
      .join("\n");
    let derived_table = format!("(\n{indented}\n  )");

    Ok(self.dialect.render_table_reference(&derived_table, &alias))
  }

  pub(super) fn into_bindings(self) -> Vec<ParameterBinding> {
    self.bindings
  }

  fn render_aggregate(
    &mut self,
    function: AggregateFunction,
    expression: Option<&Expression>,
  ) -> Result<String, SqlCompileError> {
    let rendered_expression = expression
      .map(|expression| self.render_value(expression))
      .transpose()?;

    if function != AggregateFunction::Count && rendered_expression.is_none() {
      return Err(SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
        message: format!(
          "aggregate function {:?} has no expression",
          function.as_str()
        ),
      }));
    }

    Ok(
      self
        .dialect
        .render_aggregate(function, rendered_expression.as_deref()),
    )
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
    let expression = self.render_value(expression)?;
    let values = self.render_values(values)?;
    Ok(self.render_in_values(&expression, &values))
  }

  fn render_in_parameter(
    &mut self,
    expression: &Expression,
    parameter: &str,
  ) -> Result<String, SqlCompileError> {
    let value_count = self
      .operation
      .parameters
      .get(parameter)
      .and_then(serde_json::Value::as_array)
      .map(Vec::len)
      .ok_or_else(|| SqlCompileError::MissingParameter(parameter.to_owned()))?;

    if value_count == 0 {
      return Ok("(1 = 0)".to_owned());
    }
    if !self.group_scopes.is_empty() {
      self.reserve_group_values(value_count)?;
    }

    let expression = self.render_value(expression)?;
    let placeholders: Result<Vec<_>, _> = (0..value_count)
      .map(|index| self.render_parameter_binding(parameter, Some(index)))
      .collect();
    Ok(self.render_in_values(&expression, &placeholders?))
  }

  fn render_in_values(&self, expression: &str, values: &[String]) -> String {
    if let Some(limit) = self.dialect.in_list_limit() {
      if values.len() > limit {
        let groups = values
          .chunks(limit)
          .map(|group| format!("({expression} IN ({}))", group.join(", ")))
          .collect::<Vec<_>>();
        return format!("({})", groups.join(" OR "));
      }
    }

    format!("({expression} IN ({}))", values.join(", "))
  }

  fn render_all_groups(
    &mut self,
    parameter: &str,
    scope: &str,
    predicate: &Expression,
  ) -> Result<String, SqlCompileError> {
    // Values belong to this operation; the registered template is never rewritten.
    let count = self
      .operation
      .parameters
      .get(parameter)
      .and_then(serde_json::Value::as_array)
      .map_or(0, Vec::len);
    if count == 0 {
      return Ok("(1 = 1)".to_owned());
    }
    if count > MAX_GROUP_EXPANSIONS - self.group_expansions {
      return Err(SqlCompileError::GroupExpansionLimitExceeded {
        limit: MAX_GROUP_EXPANSIONS,
      });
    }
    self.group_expansions += count;

    let mut predicates = Vec::with_capacity(count);
    for index in 0..count {
      self.group_scopes.push(GroupScope {
        scope: scope.to_owned(),
        parameter: parameter.to_owned(),
        index,
      });
      let rendered = self.render_predicate(predicate);
      self.group_scopes.pop();
      predicates.push(rendered?);
    }
    Ok(format!("({})", predicates.join(" AND ")))
  }

  fn group_scope(&self, scope: &str) -> Result<GroupScope, SqlCompileError> {
    self
      .group_scopes
      .iter()
      .rev()
      .find(|group| group.scope == scope)
      .cloned()
      .ok_or_else(|| {
        SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
          message: format!("group reference {scope:?} is outside its template"),
        })
      })
  }

  fn reserve_group_values(&mut self, count: usize) -> Result<(), SqlCompileError> {
    if count > MAX_GROUP_VALUE_EXPANSIONS - self.group_value_expansions {
      return Err(SqlCompileError::GroupValueExpansionLimitExceeded {
        limit: MAX_GROUP_VALUE_EXPANSIONS,
      });
    }
    self.group_value_expansions += count;
    Ok(())
  }

  fn render_group_key(&mut self, scope: &str) -> Result<String, SqlCompileError> {
    let group = self.group_scope(scope)?;
    let scalar_type = self
      .graph
      .parameter(&group.parameter)
      .and_then(|parameter| parameter.key_type)
      .ok_or_else(|| {
        SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
          message: format!("grouped parameter {:?} has no key type", group.parameter),
        })
      })?;
    self.render_binding(
      &group.parameter,
      BindingAddress::Path(vec![
        BindingPathSegment::Index(group.index),
        BindingPathSegment::Property("key".to_owned()),
      ]),
      scalar_type,
    )
  }

  fn render_in_group_values(
    &mut self,
    expression: &Expression,
    scope: &str,
  ) -> Result<String, SqlCompileError> {
    let group = self.group_scope(scope)?;
    let count = self
      .operation
      .parameters
      .get(&group.parameter)
      .and_then(|groups| groups.get(group.index))
      .and_then(|group| group.get("values"))
      .and_then(serde_json::Value::as_array)
      .map(Vec::len)
      .ok_or_else(|| SqlCompileError::MissingParameter(group.parameter.clone()))?;
    if count == 0 {
      return Ok("(1 = 0)".to_owned());
    }
    self.reserve_group_values(count)?;
    let scalar_type = self
      .graph
      .parameter(&group.parameter)
      .ok_or_else(|| SqlCompileError::MissingParameter(group.parameter.clone()))?
      .scalar_type;
    let expression = self.render_value(expression)?;
    let placeholders: Result<Vec<_>, _> = (0..count)
      .map(|index| {
        self.render_binding(
          &group.parameter,
          BindingAddress::Path(vec![
            BindingPathSegment::Index(group.index),
            BindingPathSegment::Property("values".to_owned()),
            BindingPathSegment::Index(index),
          ]),
          scalar_type,
        )
      })
      .collect();
    Ok(self.render_in_values(&expression, &placeholders?))
  }

  fn render_exists(
    &mut self,
    source: &str,
    from: Option<&str>,
    predicate: Option<&Expression>,
  ) -> Result<String, SqlCompileError> {
    let graph = self.graph;
    let from = from.unwrap_or(graph.root().key.as_str());
    let relation_indices = graph
      .relation_path_indices_between(from, source)
      .ok_or_else(|| {
        SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
          message: format!("exists expression source {source:?} is not reachable from {from:?}"),
        })
      })?;

    if relation_indices.is_empty() {
      return Err(SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
        message: format!("exists expression source {source:?} cannot be correlated from {from:?}"),
      }));
    }

    let first = &graph.definition().relations[relation_indices[0]];
    let mut sql = "EXISTS (\n    SELECT 1".to_owned();
    let mut where_predicates = Vec::new();

    if first.selection.is_some() {
      sql.push_str(&format!(
        "\n    FROM {}",
        self.dialect.render_single_row_source("__qg_seed")
      ));
      let target = self.render_selected_relation_source(first)?;
      sql.push_str(&format!("\n    CROSS APPLY {target}"));
    } else {
      sql.push_str(&format!("\n    FROM {}", self.render_source(&first.to)?));
      where_predicates.push(self.render_predicate(&first.on)?);
    }

    for relation_index in &relation_indices[1..] {
      let relation = &graph.definition().relations[*relation_index];
      if relation.selection.is_some() {
        let target = self.render_selected_relation_source(relation)?;
        sql.push_str(&format!("\n    CROSS APPLY {target}"));
      } else {
        let target = self.render_source(&relation.to)?;
        let condition = self.render_predicate(&relation.on)?;
        sql.push_str(&format!("\n    INNER JOIN {target}\n      ON {condition}"));
      }
    }

    if let Some(predicate) = predicate {
      where_predicates.push(self.render_predicate(predicate)?);
    }
    if let Some((first, rest)) = where_predicates.split_first() {
      sql.push_str(&format!("\n    WHERE {first}"));
      for predicate in rest {
        sql.push_str(&format!("\n      AND {predicate}"));
      }
    }

    sql.push_str("\n  )");
    Ok(sql)
  }

  fn render_parameter(&mut self, parameter: &str) -> Result<String, SqlCompileError> {
    if !self.operation.parameters.contains_key(parameter) {
      return Err(SqlCompileError::MissingParameter(parameter.to_owned()));
    }

    self.render_parameter_binding(parameter, None)
  }

  fn render_parameter_binding(
    &mut self,
    parameter: &str,
    index: Option<usize>,
  ) -> Result<String, SqlCompileError> {
    let definition = self
      .graph
      .parameter(parameter)
      .ok_or_else(|| SqlCompileError::MissingParameter(parameter.to_owned()))?;
    self.render_binding(
      parameter,
      index.map_or(BindingAddress::Scalar, BindingAddress::Index),
      definition.scalar_type,
    )
  }

  fn render_binding(
    &mut self,
    parameter: &str,
    address: BindingAddress,
    scalar_type: ScalarType,
  ) -> Result<String, SqlCompileError> {
    let key = BindingKey {
      parameter: parameter.to_owned(),
      address: address.clone(),
    };

    if let Some(binding_name) = self.binding_names.get(&key) {
      return Ok(self.dialect.render_placeholder(binding_name));
    }

    if let Some(limit) = self.dialect.parameter_limit() {
      if self.bindings.len() >= limit {
        return Err(SqlCompileError::TooManyParameters {
          dialect: self.dialect.name(),
          limit,
        });
      }
    }

    let binding_name = format!("p{}", self.bindings.len());
    self.binding_names.insert(key, binding_name.clone());
    self.bindings.push(ParameterBinding {
      name: binding_name.clone(),
      parameter: parameter.to_owned(),
      scalar_type,
      index: match address {
        BindingAddress::Index(index) => Some(index),
        _ => None,
      },
      path: match address {
        BindingAddress::Path(path) => Some(path),
        _ => None,
      },
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
      self.render_value(left)?,
      self.render_value(right)?
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
    Ok(format!("({} {operator})", self.render_value(expression)?))
  }

  fn render_predicate_group(
    &mut self,
    expressions: &[Expression],
    operator: &str,
  ) -> Result<String, SqlCompileError> {
    let expressions: Result<Vec<_>, _> = expressions
      .iter()
      .map(|expression| self.render_predicate(expression))
      .collect();
    let expressions = expressions?;
    Ok(format!("({})", expressions.join(&format!(" {operator} "))))
  }

  fn render_values(&mut self, expressions: &[Expression]) -> Result<Vec<String>, SqlCompileError> {
    expressions
      .iter()
      .map(|expression| self.render_value(expression))
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
