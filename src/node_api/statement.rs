use napi_derive::napi;
use query_graph_core::{
  ParameterBinding, SqlColumn, SqlProjectionObject, SqlRelation, SqlStatement,
};

#[napi(object)]
pub struct CompiledSqlStatement {
  pub sql: String,
  pub bindings: Vec<SqlBinding>,
  pub columns: Vec<CompiledSqlColumn>,
  pub objects: Vec<CompiledSqlObject>,
  pub relations: Vec<CompiledSqlRelation>,
}

impl From<SqlStatement> for CompiledSqlStatement {
  fn from(statement: SqlStatement) -> Self {
    Self {
      sql: statement.sql,
      bindings: statement
        .bindings
        .into_iter()
        .map(SqlBinding::from)
        .collect(),
      columns: statement.columns.into_iter().map(Into::into).collect(),
      objects: statement.objects.into_iter().map(Into::into).collect(),
      relations: statement.relations.into_iter().map(Into::into).collect(),
    }
  }
}

#[napi(object)]
pub struct SqlBinding {
  pub name: String,
  pub parameter: String,
  #[napi(ts_type = "import('./dsl.js').ScalarType")]
  pub scalar_type: String,
  pub index: Option<u32>,
}

impl From<ParameterBinding> for SqlBinding {
  fn from(binding: ParameterBinding) -> Self {
    Self {
      name: binding.name,
      parameter: binding.parameter,
      scalar_type: binding.scalar_type.as_str().to_owned(),
      index: binding
        .index
        .map(|index| u32::try_from(index).unwrap_or(u32::MAX)),
    }
  }
}

#[napi(object)]
pub struct CompiledSqlColumn {
  pub name: String,
  pub path: String,
  #[napi(ts_type = "import('./dsl.js').ScalarType")]
  pub scalar_type: String,
  pub nullable: bool,
  pub relations: Vec<String>,
}

impl From<SqlColumn> for CompiledSqlColumn {
  fn from(column: SqlColumn) -> Self {
    Self {
      name: column.name,
      path: column.path,
      scalar_type: column.scalar_type.as_str().to_owned(),
      nullable: column.nullable,
      relations: column.relations,
    }
  }
}

#[napi(object)]
pub struct CompiledSqlObject {
  pub path: String,
  pub presence_column: String,
}

impl From<SqlProjectionObject> for CompiledSqlObject {
  fn from(object: SqlProjectionObject) -> Self {
    Self {
      path: object.path,
      presence_column: object.presence_column,
    }
  }
}

#[napi(object)]
pub struct CompiledSqlRelation {
  pub name: String,
  pub from: String,
  pub to: String,
  #[napi(ts_type = "import('./dsl.js').RelationCardinality")]
  pub cardinality: String,
  pub required: bool,
}

impl From<SqlRelation> for CompiledSqlRelation {
  fn from(relation: SqlRelation) -> Self {
    Self {
      name: relation.name,
      from: relation.from,
      to: relation.to,
      cardinality: relation.cardinality.as_str().to_owned(),
      required: relation.required,
    }
  }
}
