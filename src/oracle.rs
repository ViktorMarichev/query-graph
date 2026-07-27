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
  sql::compile(graph, mapping, operation, &OracleDialect)
}

struct OracleDialect;

impl SqlDialect for OracleDialect {
  fn name(&self) -> &'static str {
    "Oracle"
  }

  fn quote_identifier(&self, identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
  }

  fn render_table_name(&self, table: &TableName) -> Result<String, SqlCompileError> {
    match table {
      TableName::Name(name) => Ok(self.quote_identifier(name)),
      TableName::Qualified {
        catalog: Some(_), ..
      } => Err(SqlCompileError::UnsupportedTableQualifier {
        dialect: self.name(),
        qualifier: "catalog",
      }),
      TableName::Qualified {
        catalog: None,
        schema,
        name,
      } => {
        let name = self.quote_identifier(name);
        Ok(match schema {
          None => name,
          Some(schema) => format!("{}.{name}", self.quote_identifier(schema)),
        })
      }
    }
  }

  fn render_table_reference(&self, table: &str, alias: &str) -> String {
    format!("{table} {}", self.quote_identifier(alias))
  }

  fn render_placeholder(&self, binding_name: &str) -> String {
    format!(":{binding_name}")
  }

  fn render_literal(&self, value: &LiteralValue) -> Result<String, SqlCompileError> {
    sql::render_literal(value, "")
  }

  fn render_function(&self, name: &str, arguments: &[String]) -> Result<String, SqlCompileError> {
    match name {
      "lower" if arguments.len() == 1 => Ok(format!("LOWER({})", arguments[0])),
      "upper" if arguments.len() == 1 => Ok(format!("UPPER({})", arguments[0])),
      "lower" | "upper" => Err(invalid_function_arity(name, "1", arguments.len())),
      "coalesce" if arguments.len() >= 2 => Ok(format!("COALESCE({})", arguments.join(", "))),
      "coalesce" => Err(invalid_function_arity(name, "at least 2", arguments.len())),
      "concat" if !arguments.is_empty() => Ok(format!("({})", arguments.join(" || "))),
      "concat" => Err(invalid_function_arity(name, "at least 1", arguments.len())),
      _ => Err(SqlCompileError::UnsupportedFunction {
        dialect: self.name(),
        function: name.to_owned(),
      }),
    }
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
    let nulls = match nulls {
      None => "",
      Some(NullsOrder::First) => " NULLS FIRST",
      Some(NullsOrder::Last) => " NULLS LAST",
    };

    format!("{expression} {direction}{nulls}")
  }

  fn render_pagination(&self, offset: Option<u64>, limit: Option<u64>) -> String {
    let mut sql = format!("\nOFFSET {} ROWS", offset.unwrap_or_default());
    if let Some(limit) = limit {
      sql.push_str(&format!(" FETCH NEXT {limit} ROWS ONLY"));
    }
    sql
  }
}
