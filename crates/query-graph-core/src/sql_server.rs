use serde::{Deserialize, Serialize};

use crate::{
  sql::{self, SqlDialect},
  CompiledGraph, CompiledRelationalMapping, LiteralValue, NullsOrder, OrderDirection,
  QueryOperation, SemanticFunction, SqlCompileError, SqlStatement, TableName,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlServerVersion {
  #[serde(rename = "2008")]
  V2008,
  #[default]
  #[serde(rename = "2012")]
  V2012,
  #[serde(rename = "2016")]
  V2016,
  #[serde(rename = "2019")]
  V2019,
  #[serde(rename = "2022")]
  V2022,
}

impl SqlServerVersion {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::V2008 => "2008",
      Self::V2012 => "2012",
      Self::V2016 => "2016",
      Self::V2019 => "2019",
      Self::V2022 => "2022",
    }
  }

  const fn supports_offset_fetch(self) -> bool {
    !matches!(self, Self::V2008)
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SqlServerCompiler {
  version: SqlServerVersion,
}

impl SqlServerCompiler {
  pub const fn new(version: SqlServerVersion) -> Self {
    Self { version }
  }

  pub const fn version(self) -> SqlServerVersion {
    self.version
  }

  pub fn compile(
    &self,
    graph: &CompiledGraph,
    mapping: &CompiledRelationalMapping,
    operation: &QueryOperation,
  ) -> Result<SqlStatement, SqlCompileError> {
    sql::compile(
      graph,
      mapping,
      operation,
      &SqlServerDialect {
        version: self.version,
      },
    )
  }
}

struct SqlServerDialect {
  version: SqlServerVersion,
}

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

  fn render_literal(&self, value: &LiteralValue) -> String {
    sql::render_literal(value, "N")
  }

  fn render_function(&self, function: SemanticFunction, arguments: &[String]) -> String {
    if function == SemanticFunction::Concat && self.version == SqlServerVersion::V2008 {
      return format!(
        "({})",
        arguments
          .iter()
          .map(|argument| format!("COALESCE({argument}, N'')"))
          .collect::<Vec<_>>()
          .join(" + ")
      );
    }

    let sql_name = match function {
      SemanticFunction::Lower => "LOWER",
      SemanticFunction::Upper => "UPPER",
      SemanticFunction::Coalesce => "COALESCE",
      SemanticFunction::Concat => "CONCAT",
    };

    format!("{sql_name}({})", arguments.join(", "))
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

  fn render_first_by_query(
    &self,
    projection: &str,
    source: &str,
    predicate: &str,
    order_by: &[String],
  ) -> Result<String, SqlCompileError> {
    Ok(format!(
      "SELECT TOP (1) {projection}\nFROM {source}\nWHERE {predicate}\nORDER BY {}",
      order_by.join(", ")
    ))
  }

  fn render_single_row_source(&self, alias: &str) -> String {
    format!(
      "(VALUES (1)) AS {}({})",
      self.quote_identifier(alias),
      self.quote_identifier("value")
    )
  }

  fn render_pagination(
    &self,
    offset: Option<u64>,
    limit: Option<u64>,
  ) -> Result<String, SqlCompileError> {
    if !self.version.supports_offset_fetch() {
      return Err(SqlCompileError::UnsupportedDialectFeature {
        dialect: self.name(),
        version: self.version.as_str(),
        feature: "OFFSET/FETCH pagination",
      });
    }

    let mut sql = format!("\nOFFSET {} ROWS", offset.unwrap_or_default());
    if let Some(limit) = limit {
      sql.push_str(&format!(" FETCH NEXT {limit} ROWS ONLY"));
    }
    Ok(sql)
  }
}
