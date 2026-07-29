use std::error::Error;
use std::fmt;

use crate::{
  planner, AggregateFunction, CompiledGraph, CompiledRelationalMapping, LiteralValue, NullsOrder,
  OrderDirection, PlanError, QueryOperation, RelationCardinality, ScalarType, SemanticFunction,
  TableName,
};

use self::renderer::Renderer;

mod renderer;

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
  pub index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlColumn {
  pub name: String,
  pub path: String,
  pub scalar_type: ScalarType,
  pub nullable: bool,
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

pub(super) trait SqlDialect {
  fn name(&self) -> &'static str;

  fn quote_identifier(&self, identifier: &str) -> String;

  fn render_table_name(&self, table: &TableName) -> Result<String, SqlCompileError>;

  fn render_table_reference(&self, table: &str, alias: &str) -> String;

  fn render_placeholder(&self, binding_name: &str) -> String;

  fn render_literal(&self, value: &LiteralValue) -> String;

  fn render_function(&self, function: SemanticFunction, arguments: &[String]) -> String;

  fn render_aggregate(&self, function: AggregateFunction, expression: Option<&str>) -> String;

  fn render_order(
    &self,
    expression: &str,
    direction: OrderDirection,
    nulls: Option<NullsOrder>,
  ) -> String;

  fn render_first_by_query(
    &self,
    projection: &str,
    source: &str,
    predicate: &str,
    order_by: &[String],
  ) -> Result<String, SqlCompileError>;

  fn render_single_row_source(&self, alias: &str) -> String;

  fn render_pagination(
    &self,
    offset: Option<u64>,
    limit: Option<u64>,
  ) -> Result<String, SqlCompileError>;
}

pub(crate) fn compile(
  graph: &CompiledGraph,
  mapping: &CompiledRelationalMapping,
  operation: &QueryOperation,
  dialect: &impl SqlDialect,
) -> Result<SqlStatement, SqlCompileError> {
  let plan = planner::build(graph, operation)?;
  let projection_indices = plan.projection_indices();
  let mut renderer = Renderer::new(graph, mapping, operation, dialect);

  let columns: Vec<_> = projection_indices
    .iter()
    .enumerate()
    .map(|(index, projection_index)| {
      let projection = &graph.definition().projection.fields[*projection_index];
      let expression_type = graph.projection_type_at(*projection_index);
      let relations = graph
        .projection_relation_path_indices(*projection_index)
        .iter()
        .map(|relation_index| graph.definition().relations[*relation_index].name.clone())
        .collect();

      SqlColumn {
        name: format!("c{index}"),
        path: projection.path.join("."),
        scalar_type: expression_type.scalar_type,
        nullable: expression_type.nullable,
        relations,
      }
    })
    .collect();

  let select_items: Result<Vec<String>, SqlCompileError> = projection_indices
    .iter()
    .zip(&columns)
    .map(|(projection_index, column)| {
      let projection = &graph.definition().projection.fields[*projection_index];
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
    if relation.selection.is_some() {
      let apply_type = if graph.relation_is_effectively_required(*relation_index) {
        "CROSS APPLY"
      } else {
        "OUTER APPLY"
      };
      let target = renderer.render_selected_relation_source(relation)?;
      sql.push_str(&format!("\n{apply_type} {target}"));
      continue;
    }

    let join_type = if graph.relation_is_effectively_required(*relation_index) {
      "INNER JOIN"
    } else {
      "LEFT JOIN"
    };
    let target = renderer.render_source(&relation.to)?;
    let condition = renderer.render_expression(&relation.on)?;
    sql.push_str(&format!("\n{join_type} {target}\n  ON {condition}"));
  }

  if !plan.pre_aggregation_constraint_indices().is_empty() {
    let predicates: Result<Vec<_>, _> = plan
      .pre_aggregation_constraint_indices()
      .iter()
      .map(|index| renderer.render_expression(&graph.definition().constraints[*index].predicate))
      .collect();
    sql.push_str(&format!("\nWHERE\n  {}", predicates?.join("\n  AND ")));
  }

  if graph.is_summary() && !graph.dimension_projection_indices().is_empty() {
    let dimensions: Result<Vec<_>, _> = graph
      .dimension_projection_indices()
      .iter()
      .map(|index| renderer.render_expression(&graph.projection_at(*index).expression))
      .collect();
    sql.push_str(&format!("\nGROUP BY\n  {}", dimensions?.join(",\n  ")));
  }

  if !plan.post_aggregation_constraint_indices().is_empty() {
    let predicates: Result<Vec<_>, _> = plan
      .post_aggregation_constraint_indices()
      .iter()
      .map(|index| renderer.render_expression(&graph.definition().constraints[*index].predicate))
      .collect();
    sql.push_str(&format!("\nHAVING\n  {}", predicates?.join("\n  AND ")));
  }

  let order_by = plan.order_by(graph);
  if !order_by.is_empty() {
    let order_items: Result<Vec<_>, _> = order_by
      .iter()
      .map(|order| renderer.render_order(order))
      .collect();
    sql.push_str(&format!("\nORDER BY\n  {}", order_items?.join(",\n  ")));
  }

  if plan.offset().is_some() || plan.limit().is_some() {
    if order_by.is_empty() {
      return Err(SqlCompileError::PaginationRequiresOrder {
        dialect: dialect.name(),
      });
    }

    sql.push_str(&dialect.render_pagination(plan.offset(), plan.limit())?);
  }

  let relations = plan
    .relation_indices()
    .iter()
    .map(|index| &graph.definition().relations[*index])
    .zip(plan.relation_indices())
    .map(|(relation, index)| SqlRelation {
      name: relation.name.clone(),
      from: relation.from.clone(),
      to: relation.to.clone(),
      cardinality: relation.cardinality,
      required: graph.relation_is_effectively_required(*index),
    })
    .collect();

  Ok(SqlStatement {
    sql,
    bindings: renderer.into_bindings(),
    columns,
    relations,
  })
}

#[derive(Debug)]
pub enum SqlCompileError {
  Plan(PlanError),
  MissingSourceMapping(String),
  MissingParameter(String),
  UnsupportedTableQualifier {
    dialect: &'static str,
    qualifier: &'static str,
  },
  UnsupportedDialectFeature {
    dialect: &'static str,
    version: &'static str,
    feature: &'static str,
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
      Self::UnsupportedTableQualifier { dialect, qualifier } => write!(
        formatter,
        "{dialect} does not support the relational mapping table qualifier {qualifier:?}"
      ),
      Self::UnsupportedDialectFeature {
        dialect,
        version,
        feature,
      } => write!(formatter, "{dialect} {version} does not support {feature}"),
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

pub(crate) fn render_literal(value: &LiteralValue, string_prefix: &str) -> String {
  match value {
    LiteralValue::Null => "NULL".to_owned(),
    LiteralValue::Boolean(value) => if *value { "1" } else { "0" }.to_owned(),
    LiteralValue::Integer(value) => value.to_string(),
    LiteralValue::Decimal(value) => value.clone(),
    LiteralValue::String(value) => format!("{string_prefix}'{}'", value.replace('\'', "''")),
  }
}
