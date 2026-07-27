use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::{
  planner, CompiledGraph, CompiledRelationalMapping, Expression, LiteralValue, NullsOrder,
  OrderDirection, ParameterCardinality, PlanError, QueryOperation, TableName,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlStatement {
  pub sql: String,
  pub bindings: Vec<ParameterBinding>,
  pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterBinding {
  pub name: String,
  pub parameter: String,
}

pub(crate) fn compile(
  graph: &CompiledGraph,
  mapping: &CompiledRelationalMapping,
  operation: &QueryOperation,
) -> Result<SqlStatement, SqlCompileError> {
  let plan = planner::build(graph, operation)?;
  let projections: Vec<_> = plan
    .projection_indices()
    .iter()
    .map(|index| &graph.definition().projection.fields[*index])
    .collect();
  let mut renderer = SqlServerRenderer::new(graph, mapping, operation);

  let fields: Vec<_> = projections
    .iter()
    .map(|projection| projection.path.join("."))
    .collect();
  let select_items: Result<Vec<String>, SqlCompileError> = projections
    .iter()
    .map(|projection| {
      Ok(format!(
        "  {} AS {}",
        renderer.render_expression(&projection.expression)?,
        quote_identifier(&projection.path.join("."))
      ))
    })
    .collect();

  let root = graph.root();
  let mut sql = format!(
    "SELECT\n{}\nFROM {} AS {}",
    select_items?.join(",\n"),
    renderer.render_table(&root.key)?,
    quote_identifier(&root.key)
  );

  for relation_index in plan.relation_indices() {
    let relation = &graph.definition().relations[*relation_index];
    let join_type = if relation.required {
      "INNER JOIN"
    } else {
      "LEFT JOIN"
    };
    let target = renderer.render_table(&relation.to)?;
    let condition = renderer.render_expression(&relation.on)?;
    sql.push_str(&format!(
      "\n{join_type} {target} AS {}\n  ON {condition}",
      quote_identifier(&relation.to)
    ));
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
      return Err(SqlCompileError::PaginationRequiresOrder);
    }

    sql.push_str(&format!(
      "\nOFFSET {} ROWS",
      plan.offset().unwrap_or_default()
    ));
    if let Some(limit) = plan.limit() {
      sql.push_str(&format!(" FETCH NEXT {limit} ROWS ONLY"));
    }
  }

  Ok(SqlStatement {
    sql,
    bindings: renderer.bindings,
    fields,
  })
}

struct SqlServerRenderer<'a> {
  graph: &'a CompiledGraph,
  mapping: &'a CompiledRelationalMapping,
  operation: &'a QueryOperation,
  binding_names: HashMap<String, String>,
  bindings: Vec<ParameterBinding>,
}

impl<'a> SqlServerRenderer<'a> {
  fn new(
    graph: &'a CompiledGraph,
    mapping: &'a CompiledRelationalMapping,
    operation: &'a QueryOperation,
  ) -> Self {
    Self {
      graph,
      mapping,
      operation,
      binding_names: HashMap::new(),
      bindings: Vec::new(),
    }
  }

  fn render_table(&self, source: &str) -> Result<String, SqlCompileError> {
    let source_mapping = self
      .mapping
      .source(source)
      .ok_or_else(|| SqlCompileError::MissingSourceMapping(source.to_owned()))?;

    Ok(render_table_name(&source_mapping.table))
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
          quote_identifier(source),
          quote_identifier(column)
        ))
      }
      Expression::Parameter { name } => self.render_parameter(name),
      Expression::Literal { value } => render_literal(value),
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
      Expression::Function { name, arguments } => self.render_function(name, arguments),
    }
  }

  fn render_parameter(&mut self, parameter: &str) -> Result<String, SqlCompileError> {
    let definition = self
      .graph
      .parameter(parameter)
      .ok_or_else(|| SqlCompileError::MissingParameter(parameter.to_owned()))?;

    if definition.cardinality == ParameterCardinality::Many {
      return Err(SqlCompileError::UnsupportedManyParameter(
        parameter.to_owned(),
      ));
    }

    if !self.operation.parameters.contains_key(parameter) {
      return Err(SqlCompileError::MissingParameter(parameter.to_owned()));
    }

    if let Some(binding_name) = self.binding_names.get(parameter) {
      return Ok(format!("@{binding_name}"));
    }

    let binding_name = format!("p{}", self.bindings.len());
    self
      .binding_names
      .insert(parameter.to_owned(), binding_name.clone());
    self.bindings.push(ParameterBinding {
      name: binding_name.clone(),
      parameter: parameter.to_owned(),
    });

    Ok(format!("@{binding_name}"))
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

  fn render_function(
    &mut self,
    name: &str,
    arguments: &[Expression],
  ) -> Result<String, SqlCompileError> {
    let sql_name = match name {
      "lower" => {
        require_arity(name, arguments, 1)?;
        "LOWER"
      }
      "upper" => {
        require_arity(name, arguments, 1)?;
        "UPPER"
      }
      "coalesce" if arguments.len() >= 2 => "COALESCE",
      "concat" if !arguments.is_empty() => "CONCAT",
      "coalesce" | "concat" => {
        return Err(SqlCompileError::InvalidFunctionArity {
          function: name.to_owned(),
          expected: if name == "coalesce" {
            "at least 2"
          } else {
            "at least 1"
          },
          actual: arguments.len(),
        });
      }
      _ => return Err(SqlCompileError::UnsupportedFunction(name.to_owned())),
    };

    let arguments: Result<Vec<_>, _> = arguments
      .iter()
      .map(|argument| self.render_expression(argument))
      .collect();
    Ok(format!("{sql_name}({})", arguments?.join(", ")))
  }

  fn render_order(&mut self, order: &crate::OrderByDefinition) -> Result<String, SqlCompileError> {
    let expression = self.render_expression(&order.expression)?;
    let direction = match order.direction {
      OrderDirection::Asc => "ASC",
      OrderDirection::Desc => "DESC",
    };

    match order.nulls {
      None => Ok(format!("{expression} {direction}")),
      Some(nulls) => {
        let null_rank = match nulls {
          NullsOrder::First => (0, 1),
          NullsOrder::Last => (1, 0),
        };
        Ok(format!(
          "CASE WHEN {expression} IS NULL THEN {} ELSE {} END ASC, {expression} {direction}",
          null_rank.0, null_rank.1
        ))
      }
    }
  }
}

#[derive(Debug)]
pub enum SqlCompileError {
  Plan(PlanError),
  MissingSourceMapping(String),
  MissingParameter(String),
  UnsupportedManyParameter(String),
  UnsupportedFunction(String),
  InvalidFunctionArity {
    function: String,
    expected: &'static str,
    actual: usize,
  },
  InvalidDecimalLiteral(String),
  PaginationRequiresOrder,
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
      Self::UnsupportedManyParameter(parameter) => write!(
        formatter,
        "many-valued parameter {parameter:?} is not supported by the SQL Server compiler yet"
      ),
      Self::UnsupportedFunction(function) => {
        write!(
          formatter,
          "semantic function {function:?} is not supported by SQL Server"
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
      Self::PaginationRequiresOrder => {
        write!(formatter, "SQL Server pagination requires a default order")
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

fn render_table_name(table: &TableName) -> String {
  match table {
    TableName::Name(name) => quote_identifier(name),
    TableName::Qualified {
      catalog,
      schema,
      name,
    } => {
      let name = quote_identifier(name);
      match (catalog, schema) {
        (None, None) => name,
        (None, Some(schema)) => format!("{}.{name}", quote_identifier(schema)),
        (Some(catalog), None) => format!("{}..{name}", quote_identifier(catalog)),
        (Some(catalog), Some(schema)) => {
          format!(
            "{}.{}.{name}",
            quote_identifier(catalog),
            quote_identifier(schema)
          )
        }
      }
    }
  }
}

fn quote_identifier(identifier: &str) -> String {
  format!("[{}]", identifier.replace(']', "]]"))
}

fn render_literal(value: &LiteralValue) -> Result<String, SqlCompileError> {
  match value {
    LiteralValue::Null => Ok("NULL".to_owned()),
    LiteralValue::Boolean(value) => Ok(if *value { "1" } else { "0" }.to_owned()),
    LiteralValue::Integer(value) => Ok(value.to_string()),
    LiteralValue::Decimal(value) if is_decimal(value) => Ok(value.clone()),
    LiteralValue::Decimal(value) => Err(SqlCompileError::InvalidDecimalLiteral(value.clone())),
    LiteralValue::String(value) => Ok(format!("N'{}'", value.replace('\'', "''"))),
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

fn is_decimal(value: &str) -> bool {
  let value = value.strip_prefix('-').unwrap_or(value);
  let mut has_digit = false;
  let mut has_decimal_point = false;

  for character in value.chars() {
    if character.is_ascii_digit() {
      has_digit = true;
    } else if character == '.' && !has_decimal_point {
      has_decimal_point = true;
    } else {
      return false;
    }
  }

  has_digit
}

fn require_arity(
  function: &str,
  arguments: &[Expression],
  expected: usize,
) -> Result<(), SqlCompileError> {
  if arguments.len() == expected {
    Ok(())
  } else {
    Err(SqlCompileError::InvalidFunctionArity {
      function: function.to_owned(),
      expected: match expected {
        1 => "1",
        _ => "exactly the declared number of",
      },
      actual: arguments.len(),
    })
  }
}
