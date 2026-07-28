use serde::{Deserialize, Serialize};

use crate::{
  sql::{self, SqlDialect},
  CompiledGraph, CompiledRelationalMapping, LiteralValue, NullsOrder, OrderDirection,
  QueryOperation, SemanticFunction, SqlCompileError, SqlStatement, TableName,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleVersion {
  #[serde(rename = "11g")]
  V11g,
  #[default]
  #[serde(rename = "12c")]
  V12c,
  #[serde(rename = "19c")]
  V19c,
  #[serde(rename = "21c")]
  V21c,
  #[serde(rename = "23ai")]
  V23ai,
}

impl OracleVersion {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::V11g => "11g",
      Self::V12c => "12c",
      Self::V19c => "19c",
      Self::V21c => "21c",
      Self::V23ai => "23ai",
    }
  }

  const fn supports_offset_fetch(self) -> bool {
    !matches!(self, Self::V11g)
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OracleCompiler {
  version: OracleVersion,
}

impl OracleCompiler {
  pub const fn new(version: OracleVersion) -> Self {
    Self { version }
  }

  pub const fn version(self) -> OracleVersion {
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
      &OracleDialect {
        version: self.version,
      },
    )
  }
}

struct OracleDialect {
  version: OracleVersion,
}

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

  fn render_literal(&self, value: &LiteralValue) -> String {
    sql::render_literal(value, "")
  }

  fn render_function(&self, function: SemanticFunction, arguments: &[String]) -> String {
    match function {
      SemanticFunction::Lower => format!("LOWER({})", arguments[0]),
      SemanticFunction::Upper => format!("UPPER({})", arguments[0]),
      SemanticFunction::Coalesce => format!("COALESCE({})", arguments.join(", ")),
      SemanticFunction::Concat => format!("({})", arguments.join(" || ")),
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

  fn render_first_by_query(
    &self,
    projection: &str,
    source: &str,
    predicate: &str,
    order_by: &[String],
  ) -> Result<String, SqlCompileError> {
    if self.version == OracleVersion::V11g {
      return Err(SqlCompileError::UnsupportedDialectFeature {
        dialect: self.name(),
        version: self.version.as_str(),
        feature: "firstBy relation selection",
      });
    }

    Ok(format!(
      "SELECT {projection}\nFROM {source}\nWHERE {predicate}\nORDER BY {}\nFETCH FIRST 1 ROW ONLY",
      order_by.join(", ")
    ))
  }

  fn render_single_row_source(&self, alias: &str) -> String {
    format!("DUAL {}", self.quote_identifier(alias))
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
