use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::{
  planner, scalar::is_decimal_text, CompiledGraph, CompiledRelationalMapping, Expression,
  LiteralValue, NullsOrder, OrderDirection, ParameterCardinality, PlanError, QueryOperation,
  RelationCardinality, ScalarType, TableName,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlStatement {
  pub sql: String,
  pub bindings: Vec<ParameterBinding>,
  pub columns: Vec<SqlColumn>,
  pub relations: Vec<SqlRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterBinding {
  pub name: String,
  pub parameter: String,
  pub scalar_type: ScalarType,
  pub cardinality: ParameterCardinality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlColumn {
  pub name: String,
  pub path: String,
  pub relations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlRelation {
  pub name: String,
  pub from: String,
  pub to: String,
  pub cardinality: RelationCardinality,
  pub required: bool,
}

pub(crate) trait SqlDialect {
  fn name(&self) -> &'static str;

  fn quote_identifier(&self, identifier: &str) -> String;

  fn render_table_name(&self, table: &TableName) -> Result<String, SqlCompileError>;

  fn render_table_reference(&self, table: &str, alias: &str) -> String;

  fn render_placeholder(&self, binding_name: &str) -> String;

  fn render_literal(&self, value: &LiteralValue) -> Result<String, SqlCompileError>;

  fn render_function(&self, name: &str, arguments: &[String]) -> Result<String, SqlCompileError>;

  fn render_order(
    &self,
    expression: &str,
    direction: OrderDirection,
    nulls: Option<NullsOrder>,
  ) -> String;

  fn render_pagination(&self, offset: Option<u64>, limit: Option<u64>) -> String;
}

pub(crate) fn compile(
  graph: &CompiledGraph,
  mapping: &CompiledRelationalMapping,
  operation: &QueryOperation,
  dialect: &impl SqlDialect,
) -> Result<SqlStatement, SqlCompileError> {
  let plan = planner::build(graph, operation)?;
  let projections: Vec<_> = plan
    .projection_indices()
    .iter()
    .map(|index| &graph.definition().projection.fields[*index])
    .collect();
  let mut renderer = Renderer::new(graph, mapping, operation, dialect);

  let columns: Vec<_> = projections
    .iter()
    .enumerate()
    .map(|(index, projection)| SqlColumn {
      name: format!("c{index}"),
      path: projection.path.join("."),
      relations: projection.relations.clone(),
    })
    .collect();
  let select_items: Result<Vec<String>, SqlCompileError> = projections
    .iter()
    .zip(&columns)
    .map(|(projection, column)| {
      Ok(format!(
        "  {} AS {}",
        renderer.render_expression(&projection.expression)?,
        dialect.quote_identifier(&column.name)
      ))
    })
    .collect();

  let root = graph.root();
  let mut sql = format!(
    "SELECT\n{}\nFROM {}",
    select_items?.join(",\n"),
    renderer.render_source(&root.key)?
  );

  for relation_index in plan.relation_indices() {
    let relation = &graph.definition().relations[*relation_index];
    let join_type = if relation.required {
      "INNER JOIN"
    } else {
      "LEFT JOIN"
    };
    let target = renderer.render_source(&relation.to)?;
    let condition = renderer.render_expression(&relation.on)?;
    sql.push_str(&format!("\n{join_type} {target}\n  ON {condition}"));
  }

  if !plan.constraint_indices().is_empty() {
    let predicates: Result<Vec<_>, _> = plan
      .constraint_indices()
      .iter()
      .map(|index| renderer.render_expression(&graph.definition().constraints[*index].predicate))
      .collect();
    sql.push_str(&format!("\nWHERE\n  {}", predicates?.join("\n  AND ")));
  }

  if !graph.definition().default_order_by.is_empty() {
    let order_items: Result<Vec<_>, _> = graph
      .definition()
      .default_order_by
      .iter()
      .map(|order| renderer.render_order(order))
      .collect();
    sql.push_str(&format!("\nORDER BY\n  {}", order_items?.join(",\n  ")));
  }

  if plan.offset().is_some() || plan.limit().is_some() {
    if graph.definition().default_order_by.is_empty() {
      return Err(SqlCompileError::PaginationRequiresOrder {
        dialect: dialect.name(),
      });
    }

    sql.push_str(&dialect.render_pagination(plan.offset(), plan.limit()));
  }

  let relations = plan
    .relation_indices()
    .iter()
    .map(|index| &graph.definition().relations[*index])
    .map(|relation| SqlRelation {
      name: relation.name.clone(),
      from: relation.from.clone(),
      to: relation.to.clone(),
      cardinality: relation.cardinality,
      required: relation.required,
    })
    .collect();

  Ok(SqlStatement {
    sql,
    bindings: renderer.bindings,
    columns,
    relations,
  })
}

struct Renderer<'a, D> {
  graph: &'a CompiledGraph,
  mapping: &'a CompiledRelationalMapping,
  operation: &'a QueryOperation,
  dialect: &'a D,
  binding_names: HashMap<String, String>,
  bindings: Vec<ParameterBinding>,
}

impl<'a, D: SqlDialect> Renderer<'a, D> {
  fn new(
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

  fn render_source(&self, source: &str) -> Result<String, SqlCompileError> {
    let source_mapping = self
      .mapping
      .source(source)
      .ok_or_else(|| SqlCompileError::MissingSourceMapping(source.to_owned()))?;
    let table = self.dialect.render_table_name(&source_mapping.table)?;

    let alias = self.source_alias(source)?;
    Ok(self.dialect.render_table_reference(&table, &alias))
  }

  fn render_expression(&mut self, expression: &Expression) -> Result<String, SqlCompileError> {
    match expression {
      Expression::Field { source, field } => {
        let column = self
          .mapping
          .column(source, field)
          .ok_or_else(|| SqlCompileError::MissingSourceMapping(source.clone()))?;
        Ok(format!(
          "{}.{}",
          self.dialect.quote_identifier(&self.source_alias(source)?),
          self.dialect.quote_identifier(column)
        ))
      }
      Expression::Parameter { name } => self.render_parameter(name),
      Expression::Literal { value } => self.dialect.render_literal(value),
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
      Expression::In { expression, values } => {
        let expression = self.render_expression(expression)?;
        let values: Result<Vec<_>, _> = values
          .iter()
          .map(|value| self.render_expression(value))
          .collect();
        Ok(format!("({expression} IN ({}))", values?.join(", ")))
      }
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
      Expression::Function { name, arguments } => {
        let arguments: Result<Vec<_>, _> = arguments
          .iter()
          .map(|argument| self.render_expression(argument))
          .collect();
        self.dialect.render_function(name, &arguments?)
      }
    }
  }

  fn render_parameter(&mut self, parameter: &str) -> Result<String, SqlCompileError> {
    let definition = self
      .graph
      .parameter(parameter)
      .ok_or_else(|| SqlCompileError::MissingParameter(parameter.to_owned()))?;

    if definition.cardinality == ParameterCardinality::Many {
      return Err(SqlCompileError::UnsupportedManyParameter {
        dialect: self.dialect.name(),
        parameter: parameter.to_owned(),
      });
    }

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
      cardinality: definition.cardinality,
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
      let null_operator = if negated_null {
        "IS NOT NULL"
      } else {
        "IS NULL"
      };
      return Ok(format!(
        "({} {null_operator})",
        self.render_expression(left)?
      ));
    }

    if is_null_literal(left) {
      let null_operator = if negated_null {
        "IS NOT NULL"
      } else {
        "IS NULL"
      };
      return Ok(format!(
        "({} {null_operator})",
        self.render_expression(right)?
      ));
    }

    self.render_binary(left, operator, right)
  }

  fn render_expression_group(
    &mut self,
    expressions: &[Expression],
    operator: &str,
  ) -> Result<String, SqlCompileError> {
    let expressions: Result<Vec<_>, _> = expressions
      .iter()
      .map(|expression| self.render_expression(expression))
      .collect();
    Ok(format!("({})", expressions?.join(&format!(" {operator} "))))
  }

  fn render_order(&mut self, order: &crate::OrderByDefinition) -> Result<String, SqlCompileError> {
    let expression = self.render_expression(&order.expression)?;
    Ok(
      self
        .dialect
        .render_order(&expression, order.direction, order.nulls),
    )
  }
}

#[derive(Debug)]
pub enum SqlCompileError {
  Plan(PlanError),
  MissingSourceMapping(String),
  MissingParameter(String),
  UnsupportedManyParameter {
    dialect: &'static str,
    parameter: String,
  },
  UnsupportedFunction {
    dialect: &'static str,
    function: String,
  },
  InvalidFunctionArity {
    function: String,
    expected: &'static str,
    actual: usize,
  },
  InvalidDecimalLiteral(String),
  UnsupportedTableQualifier {
    dialect: &'static str,
    qualifier: &'static str,
  },
  PaginationRequiresOrder {
    dialect: &'static str,
  },
}

impl fmt::Display for SqlCompileError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Plan(error) => error.fmt(formatter),
      Self::MissingSourceMapping(source) => {
        write!(formatter, "source {source:?} has no relational mapping")
      }
      Self::MissingParameter(parameter) => {
        write!(
          formatter,
          "parameter {parameter:?} is required by the SQL plan"
        )
      }
      Self::UnsupportedManyParameter { dialect, parameter } => write!(
        formatter,
        "many-valued parameter {parameter:?} is not supported by the {dialect} compiler yet"
      ),
      Self::UnsupportedFunction { dialect, function } => {
        write!(
          formatter,
          "semantic function {function:?} is not supported by {dialect}"
        )
      }
      Self::InvalidFunctionArity {
        function,
        expected,
        actual,
      } => write!(
        formatter,
        "semantic function {function:?} expects {expected} argument(s), received {actual}"
      ),
      Self::InvalidDecimalLiteral(value) => {
        write!(formatter, "decimal literal {value:?} is invalid")
      }
      Self::UnsupportedTableQualifier { dialect, qualifier } => write!(
        formatter,
        "{dialect} does not support the relational mapping table qualifier {qualifier:?}"
      ),
      Self::PaginationRequiresOrder { dialect } => {
        write!(formatter, "{dialect} pagination requires a default order")
      }
    }
  }
}

impl Error for SqlCompileError {}

impl From<PlanError> for SqlCompileError {
  fn from(error: PlanError) -> Self {
    Self::Plan(error)
  }
}

pub(crate) fn render_literal(
  value: &LiteralValue,
  string_prefix: &str,
) -> Result<String, SqlCompileError> {
  match value {
    LiteralValue::Null => Ok("NULL".to_owned()),
    LiteralValue::Boolean(value) => Ok(if *value { "1" } else { "0" }.to_owned()),
    LiteralValue::Integer(value) => Ok(value.to_string()),
    LiteralValue::Decimal(value) if is_decimal_text(value) => Ok(value.clone()),
    LiteralValue::Decimal(value) => Err(SqlCompileError::InvalidDecimalLiteral(value.clone())),
    LiteralValue::String(value) => Ok(format!("{string_prefix}'{}'", value.replace('\'', "''"))),
  }
}

pub(crate) fn invalid_function_arity(
  function: &str,
  expected: &'static str,
  actual: usize,
) -> SqlCompileError {
  SqlCompileError::InvalidFunctionArity {
    function: function.to_owned(),
    expected,
    actual,
  }
}

fn is_null_literal(expression: &Expression) -> bool {
  matches!(
    expression,
    Expression::Literal {
      value: LiteralValue::Null
    }
  )
}
