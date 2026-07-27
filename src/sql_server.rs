use crate::{
  sql::{self, invalid_function_arity, SqlDialect},
  CompiledGraph, CompiledRelationalMapping, LiteralValue, NullsOrder, OrderDirection,
  QueryOperation, SqlCompileError, SqlStatement, TableName,
};

pub(crate) fn compile(
  graph: &CompiledGraph,
  mapping: &CompiledRelationalMapping,
  operation: &QueryOperation,
) -> Result<SqlStatement, SqlCompileError> {
  sql::compile(graph, mapping, operation, &SqlServerDialect)
}

struct SqlServerDialect;

impl SqlDialect for SqlServerDialect {
  fn name(&self) -> &'static str {
    "SQL Server"
  }

  fn quote_identifier(&self, identifier: &str) -> String {
    format!("[{}]", identifier.replace(']', "]]"))
  }

  fn render_table_name(&self, table: &TableName) -> Result<String, SqlCompileError> {
    Ok(match table {
      TableName::Name(name) => self.quote_identifier(name),
      TableName::Qualified {
        catalog,
        schema,
        name,
      } => {
        let name = self.quote_identifier(name);
        match (catalog, schema) {
          (None, None) => name,
          (None, Some(schema)) => format!("{}.{name}", self.quote_identifier(schema)),
          (Some(catalog), None) => format!("{}..{name}", self.quote_identifier(catalog)),
          (Some(catalog), Some(schema)) => {
            format!(
              "{}.{}.{name}",
              self.quote_identifier(catalog),
              self.quote_identifier(schema)
            )
          }
        }
      }
    })
  }

  fn render_table_reference(&self, table: &str, alias: &str) -> String {
    format!("{table} AS {}", self.quote_identifier(alias))
  }

  fn render_placeholder(&self, binding_name: &str) -> String {
    format!("@{binding_name}")
  }

  fn render_literal(&self, value: &LiteralValue) -> Result<String, SqlCompileError> {
    sql::render_literal(value, "N")
  }

  fn render_function(&self, name: &str, arguments: &[String]) -> Result<String, SqlCompileError> {
    let sql_name = match name {
      "lower" if arguments.len() == 1 => "LOWER",
      "upper" if arguments.len() == 1 => "UPPER",
      "lower" | "upper" => {
        return Err(invalid_function_arity(name, "1", arguments.len()));
      }
      "coalesce" if arguments.len() >= 2 => "COALESCE",
      "concat" if !arguments.is_empty() => "CONCAT",
      "coalesce" | "concat" => {
        return Err(invalid_function_arity(
          name,
          if name == "coalesce" {
            "at least 2"
          } else {
            "at least 1"
          },
          arguments.len(),
        ));
      }
      _ => {
        return Err(SqlCompileError::UnsupportedFunction {
          dialect: self.name(),
          function: name.to_owned(),
        });
      }
    };

    Ok(format!("{sql_name}({})", arguments.join(", ")))
  }

  fn render_order(
    &self,
    expression: &str,
    direction: OrderDirection,
    nulls: Option<NullsOrder>,
  ) -> String {
    let direction = match direction {
      OrderDirection::Asc => "ASC",
      OrderDirection::Desc => "DESC",
    };

    match nulls {
      None => format!("{expression} {direction}"),
      Some(nulls) => {
        let null_rank = match nulls {
          NullsOrder::First => (0, 1),
          NullsOrder::Last => (1, 0),
        };
        format!(
          "CASE WHEN {expression} IS NULL THEN {} ELSE {} END ASC, {expression} {direction}",
          null_rank.0, null_rank.1
        )
      }
    }
  }

  fn render_pagination(&self, offset: Option<u64>, limit: Option<u64>) -> String {
    let mut sql = format!("\nOFFSET {} ROWS", offset.unwrap_or_default());
    if let Some(limit) = limit {
      sql.push_str(&format!(" FETCH NEXT {limit} ROWS ONLY"));
    }
    sql
  }
}
