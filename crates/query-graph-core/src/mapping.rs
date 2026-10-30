use std::collections::{hash_map::Entry, HashMap};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::CompiledGraph;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationalMapping {
  pub sources: HashMap<String, SourceMapping>,
}

impl RelationalMapping {
  pub fn merge(mappings: impl IntoIterator<Item = Self>) -> Result<Self, MappingIssues> {
    let mut sources: HashMap<String, SourceMapping> = HashMap::new();
    let mut issues = Vec::new();

    for (mapping_index, mapping) in mappings.into_iter().enumerate() {
      for (source, source_mapping) in mapping.sources {
        let Some(existing) = sources.get_mut(&source) else {
          sources.insert(source, source_mapping);
          continue;
        };

        if !existing.table.denotes_same_table(&source_mapping.table) {
          issues.push(MappingIssue::new(
            MappingIssueCode::ConflictingSourceTable,
            format!("mappings[{mapping_index}].sources.{source}.table"),
            format!(
              "source {source:?} maps to conflicting physical tables {:?} and {:?}",
              existing.table, source_mapping.table
            ),
          ));
        }

        for (field, column) in source_mapping.columns {
          match existing.columns.entry(field) {
            Entry::Vacant(entry) => {
              entry.insert(column);
            }
            Entry::Occupied(entry) if entry.get() == &column => {}
            Entry::Occupied(entry) => {
              let field = entry.key();
              issues.push(MappingIssue::new(
                MappingIssueCode::ConflictingColumn,
                format!("mappings[{mapping_index}].sources.{source}.columns.{field}"),
                format!(
                  "field {source:?}.{field:?} maps to conflicting physical columns {:?} and {:?}",
                  entry.get(),
                  column
                ),
              ));
            }
          }
        }
      }
    }

    if issues.is_empty() {
      Ok(Self { sources })
    } else {
      Err(MappingIssues(issues))
    }
  }

  pub fn compile(self, graph: &CompiledGraph) -> Result<CompiledRelationalMapping, MappingIssues> {
    let mut issues = Vec::new();

    for source in self.sources.keys() {
      if graph.source(source).is_none() {
        issues.push(MappingIssue::new(
          MappingIssueCode::UnknownSource,
          format!("sources.{source}"),
          format!("source {source:?} is not defined by the graph"),
        ));
      }
    }

    for source in &graph.definition().sources {
      let Some(source_mapping) = self.sources.get(&source.key) else {
        issues.push(MappingIssue::new(
          MappingIssueCode::MissingSource,
          format!("sources.{}", source.key),
          format!("source {:?} has no relational mapping", source.key),
        ));
        continue;
      };

      validate_table_name(&source_mapping.table, &source.key, &mut issues);

      for (field, column) in &source_mapping.columns {
        if graph.field(&source.key, field).is_none() {
          issues.push(MappingIssue::new(
            MappingIssueCode::UnknownColumnField,
            format!("sources.{}.columns.{field}", source.key),
            format!(
              "column mapping refers to unknown field {:?}.{:?}",
              source.key, field
            ),
          ));
        }

        if column.trim().is_empty() {
          issues.push(MappingIssue::new(
            MappingIssueCode::EmptyColumnName,
            format!("sources.{}.columns.{field}", source.key),
            "physical column name must not be empty",
          ));
        }
      }
    }

    if issues.is_empty() {
      Ok(CompiledRelationalMapping { mapping: self })
    } else {
      Err(MappingIssues(issues))
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceMapping {
  pub table: TableName,
  #[serde(default)]
  pub columns: HashMap<String, String>,
}

impl SourceMapping {
  pub fn new(table: impl Into<TableName>) -> Self {
    Self {
      table: table.into(),
      columns: HashMap::new(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum TableName {
  Name(String),
  Qualified {
    #[serde(default)]
    catalog: Option<String>,
    #[serde(default)]
    schema: Option<String>,
    name: String,
    #[serde(
      default,
      rename = "databaseLink",
      skip_serializing_if = "Option::is_none"
    )]
    database_link: Option<String>,
  },
}

impl TableName {
  fn denotes_same_table(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Name(left), Self::Name(right)) => left == right,
      (
        Self::Name(left),
        Self::Qualified {
          catalog: None,
          schema: None,
          name: right,
          database_link: None,
        },
      )
      | (
        Self::Qualified {
          catalog: None,
          schema: None,
          name: left,
          database_link: None,
        },
        Self::Name(right),
      ) => left == right,
      (
        Self::Qualified {
          catalog: left_catalog,
          schema: left_schema,
          name: left_name,
          database_link: left_link,
        },
        Self::Qualified {
          catalog: right_catalog,
          schema: right_schema,
          name: right_name,
          database_link: right_link,
        },
      ) => {
        left_catalog == right_catalog
          && left_schema == right_schema
          && left_name == right_name
          && match (left_link, right_link) {
            (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
            (None, None) => true,
            _ => false,
          }
      }
      (left, right) => left == right,
    }
  }
}

impl From<String> for TableName {
  fn from(name: String) -> Self {
    Self::Name(name)
  }
}

impl From<&str> for TableName {
  fn from(name: &str) -> Self {
    Self::Name(name.to_owned())
  }
}

#[derive(Debug, Clone)]
pub struct CompiledRelationalMapping {
  mapping: RelationalMapping,
}

impl CompiledRelationalMapping {
  pub fn definition(&self) -> &RelationalMapping {
    &self.mapping
  }

  pub fn source(&self, source: &str) -> Option<&SourceMapping> {
    self.mapping.sources.get(source)
  }

  pub fn column<'a>(&'a self, source: &str, field: &'a str) -> Option<&'a str> {
    let source = self.source(source)?;
    Some(source.columns.get(field).map_or(field, String::as_str))
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingIssue {
  pub code: MappingIssueCode,
  pub location: String,
  pub message: String,
}

impl MappingIssue {
  fn new(code: MappingIssueCode, location: impl Into<String>, message: impl Into<String>) -> Self {
    Self {
      code,
      location: location.into(),
      message: message.into(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MappingIssueCode {
  MissingSource,
  ConflictingSourceTable,
  ConflictingColumn,
  UnknownSource,
  EmptyTableName,
  EmptyTableQualifier,
  InvalidDatabaseLink,
  UnknownColumnField,
  EmptyColumnName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MappingIssues(Vec<MappingIssue>);

impl MappingIssues {
  pub fn as_slice(&self) -> &[MappingIssue] {
    &self.0
  }

  pub fn into_vec(self) -> Vec<MappingIssue> {
    self.0
  }
}

impl fmt::Display for MappingIssues {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "relational mapping contains {} issue(s)",
      self.0.len()
    )?;

    for issue in &self.0 {
      write!(
        formatter,
        "\n- {:?} at {}: {}",
        issue.code, issue.location, issue.message
      )?;
    }

    Ok(())
  }
}

impl Error for MappingIssues {}

fn validate_table_name(table: &TableName, source: &str, issues: &mut Vec<MappingIssue>) {
  match table {
    TableName::Name(name) => {
      if name.trim().is_empty() {
        issues.push(MappingIssue::new(
          MappingIssueCode::EmptyTableName,
          format!("sources.{source}.table"),
          "physical table name must not be empty",
        ));
      }
    }
    TableName::Qualified {
      catalog,
      schema,
      name,
      database_link,
    } => {
      if name.trim().is_empty() {
        issues.push(MappingIssue::new(
          MappingIssueCode::EmptyTableName,
          format!("sources.{source}.table.name"),
          "physical table name must not be empty",
        ));
      }

      for (qualifier, value) in [("catalog", catalog), ("schema", schema)] {
        if value.as_ref().is_some_and(|value| value.trim().is_empty()) {
          issues.push(MappingIssue::new(
            MappingIssueCode::EmptyTableQualifier,
            format!("sources.{source}.table.{qualifier}"),
            format!("table {qualifier} must not be empty when provided"),
          ));
        }
      }

      if database_link
        .as_deref()
        .is_some_and(|name| !valid_database_link(name))
      {
        issues.push(MappingIssue::new(
          MappingIssueCode::InvalidDatabaseLink,
          format!("sources.{source}.table.databaseLink"),
          "databaseLink must be an ASCII name of 1-128 bytes in LINK[.DOMAIN...][@CONNECTION] form; \
           LINK and CONNECTION must start with a letter, and each nonempty part may contain only \
           letters, digits, _, $ or #",
        ));
      }
    }
  }
}

fn valid_database_link(name: &str) -> bool {
  if name.len() > 128 || !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
    return false;
  }

  let (database, connection) = match name.split_once('@') {
    Some((database, connection)) => (database, Some(connection)),
    None => (name, None),
  };

  let valid_part = |part: &str| {
    !part.is_empty()
      && part
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'#'))
  };

  database.split('.').all(valid_part)
    && connection.is_none_or(|connection| {
      connection
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && valid_part(connection)
    })
}
