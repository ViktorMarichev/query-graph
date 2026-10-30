use serde::{Deserialize, Serialize};

use crate::{
  sql::{self, SqlDialect},
  AggregateFunction, CompiledGraph, CompiledRelationalMapping, LiteralValue, NullsOrder,
  OrderDirection, QueryOperation, SemanticFunction, SqlCompileError, SqlStatement, TableName,
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
        database_link: Some(_),
        ..
      } => {
        return Err(SqlCompileError::UnsupportedTableQualifier {
          dialect: self.name(),
          qualifier: "databaseLink",
        });
      }
      TableName::Qualified {
        catalog,
        schema,
        name,
        database_link: None,
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

  fn parameter_limit(&self) -> Option<usize> {
    Some(2100)
  }

  fn render_literal(&self, value: &LiteralValue) -> String {
    sql::render_literal(value, "N")
  }

  fn render_function(&self, function: SemanticFunction, arguments: &[String]) -> String {
    match function {
      SemanticFunction::Lower => format!("LOWER({})", arguments[0]),
      SemanticFunction::Upper => format!("UPPER({})", arguments[0]),
      SemanticFunction::Coalesce => format!("COALESCE({})", arguments.join(", ")),
      SemanticFunction::Concat if self.version == SqlServerVersion::V2008 => format!(
        "({})",
        arguments
          .iter()
          .map(|argument| format!("COALESCE({argument}, N'')"))
          .collect::<Vec<_>>()
          .join(" + ")
      ),
      SemanticFunction::Concat => render_concat(arguments),
      SemanticFunction::Divide => format!("((1.0 * {}) / {})", arguments[0], arguments[1]),
      SemanticFunction::Truncate => format!("ROUND({}, 0, 1)", arguments[0]),
      SemanticFunction::Modulo => format!("({} % {})", arguments[0], arguments[1]),
    }
  }

  fn render_aggregate(&self, function: AggregateFunction, expression: Option<&str>) -> String {
    let expression = expression.unwrap_or("*");
    match function {
      AggregateFunction::Count => format!("COUNT_BIG({expression})"),
      AggregateFunction::CountDistinct => format!("COUNT_BIG(DISTINCT {expression})"),
      AggregateFunction::Sum => format!("SUM({expression})"),
      AggregateFunction::Average => format!("AVG((1.0 * {expression}))"),
      AggregateFunction::Minimum => format!("MIN({expression})"),
      AggregateFunction::Maximum => format!("MAX({expression})"),
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

fn render_concat(arguments: &[String]) -> String {
  const MAX_ARGUMENTS: usize = 254;

  match arguments {
    [argument] => format!("CONCAT({argument}, '')"),
    _ if arguments.len() <= MAX_ARGUMENTS => format!("CONCAT({})", arguments.join(", ")),
    _ => {
      // Each group needs a LOB input so intermediate strings are not truncated before
      // they reach the outer CONCAT. Grouping also keeps the nesting depth small.
      let groups: Vec<_> = arguments
        .chunks(MAX_ARGUMENTS - 1)
        .map(|group| format!("CONCAT(CAST(N'' AS nvarchar(max)), {})", group.join(", ")))
        .collect();
      render_concat(&groups)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn concat_sql(arguments: &[String]) -> String {
    SqlServerDialect {
      version: SqlServerVersion::V2012,
    }
    .render_function(SemanticFunction::Concat, arguments)
  }

  #[test]
  fn renders_single_concat_argument_without_unicode_promotion() {
    for argument in [
      "@value",
      "NULL",
      "N'Привет'",
      "CAST(@value AS varchar(8000))",
    ] {
      assert_eq!(
        concat_sql(&[argument.into()]),
        format!("CONCAT({argument}, '')")
      );
    }
  }

  #[test]
  fn preserves_native_concat_for_supported_argument_counts() {
    for count in [2, 253, 254] {
      let arguments: Vec<_> = (0..count).map(|index| format!("@v{index}")).collect();
      assert_eq!(
        concat_sql(&arguments),
        format!("CONCAT({})", arguments.join(", "))
      );
    }
  }

  #[test]
  fn groups_concat_arguments_without_reordering_or_deep_nesting() {
    for count in [255, 506, 507, 1_000, 64_263] {
      let arguments: Vec<_> = (0..count).map(|index| format!("@v{index}")).collect();
      let sql = concat_sql(&arguments);
      assert_concat_tree(&sql, count);
      assert!(sql.contains("CAST(N'' AS nvarchar(max))"));
    }
  }

  // Inspect only this fixture's grammar: @vN arguments, CONCAT, and the empty-string CAST.
  // Count arguments independently of the grouping implementation, including leftover groups.
  fn assert_concat_tree(sql: &str, expected_arguments: usize) {
    let mut calls = Vec::new();
    let mut arguments = Vec::new();
    let mut depth = 0;
    let mut maximum_depth = 0;
    for (index, character) in sql.char_indices() {
      match character {
        '(' => {
          let is_concat = sql[..index].ends_with("CONCAT");
          if is_concat {
            depth += 1;
            maximum_depth = maximum_depth.max(depth);
          }
          calls.push((is_concat, 1));
        }
        ',' => calls.last_mut().unwrap().1 += 1,
        ')' => {
          let (is_concat, count) = calls.pop().unwrap();
          if is_concat {
            assert!(
              (2..=254).contains(&count),
              "CONCAT received {count} arguments"
            );
            depth -= 1;
          }
        }
        '@' => {
          let number: String = sql[index + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
          arguments.push(number.parse::<usize>().unwrap());
        }
        _ => {}
      }
    }
    assert!(calls.is_empty());
    assert!(
      maximum_depth <= 3,
      "unexpected CONCAT nesting depth {maximum_depth}"
    );
    assert_eq!(arguments, (0..expected_arguments).collect::<Vec<_>>());
  }
}
