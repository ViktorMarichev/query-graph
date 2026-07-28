use serde::{Deserialize, Serialize};

use crate::{CompiledGraph, DefinitionIssues, Expression};

pub const GRAPH_DEFINITION_VERSION: u32 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDefinition {
  pub schema_version: u32,
  pub name: String,
  pub root: String,
  pub sources: Vec<SourceDefinition>,
  #[serde(default)]
  pub parameters: Vec<ParameterDefinition>,
  #[serde(default)]
  pub relations: Vec<RelationDefinition>,
  #[serde(default)]
  pub constraints: Vec<ConstraintDefinition>,
  #[serde(default)]
  pub projection: ProjectionDefinition,
  #[serde(default)]
  pub default_order_by: Vec<OrderByDefinition>,
}

impl GraphDefinition {
  pub fn new(name: impl Into<String>, root: impl Into<String>) -> Self {
    Self {
      schema_version: GRAPH_DEFINITION_VERSION,
      name: name.into(),
      root: root.into(),
      sources: Vec::new(),
      parameters: Vec::new(),
      relations: Vec::new(),
      constraints: Vec::new(),
      projection: ProjectionDefinition::default(),
      default_order_by: Vec::new(),
    }
  }

  pub fn compile(self) -> Result<CompiledGraph, DefinitionIssues> {
    CompiledGraph::try_from_definition(self)
  }

  pub fn is_summary(&self) -> bool {
    self
      .projection
      .fields
      .iter()
      .any(|field| field.role != ProjectionFieldRole::Value)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDefinition {
  pub key: String,
  pub fields: Vec<FieldDefinition>,
}

impl SourceDefinition {
  pub fn new(key: impl Into<String>, fields: Vec<FieldDefinition>) -> Self {
    Self {
      key: key.into(),
      fields,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldDefinition {
  pub name: String,
  pub scalar_type: ScalarType,
  #[serde(default)]
  pub nullable: bool,
  #[serde(default = "default_true")]
  pub selectable: bool,
}

impl FieldDefinition {
  pub fn new(name: impl Into<String>, scalar_type: ScalarType) -> Self {
    Self {
      name: name.into(),
      scalar_type,
      nullable: false,
      selectable: true,
    }
  }

  pub fn nullable(mut self) -> Self {
    self.nullable = true;
    self
  }

  pub fn hidden(mut self) -> Self {
    self.selectable = false;
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScalarType {
  Boolean,
  Int32,
  Int64,
  Float64,
  Decimal,
  String,
  Date,
  DateTime,
  Binary,
  Json,
}

impl ScalarType {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Boolean => "boolean",
      Self::Int32 => "int32",
      Self::Int64 => "int64",
      Self::Float64 => "float64",
      Self::Decimal => "decimal",
      Self::String => "string",
      Self::Date => "date",
      Self::DateTime => "dateTime",
      Self::Binary => "binary",
      Self::Json => "json",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterDefinition {
  pub name: String,
  pub scalar_type: ScalarType,
  #[serde(default)]
  pub shape: ParameterShape,
  #[serde(default)]
  pub required: bool,
}

impl ParameterDefinition {
  pub fn required(name: impl Into<String>, scalar_type: ScalarType) -> Self {
    Self {
      name: name.into(),
      scalar_type,
      shape: ParameterShape::Scalar,
      required: true,
    }
  }

  pub fn optional(name: impl Into<String>, scalar_type: ScalarType) -> Self {
    Self {
      name: name.into(),
      scalar_type,
      shape: ParameterShape::Scalar,
      required: false,
    }
  }

  pub fn required_list(name: impl Into<String>, scalar_type: ScalarType) -> Self {
    Self {
      name: name.into(),
      scalar_type,
      shape: ParameterShape::List,
      required: true,
    }
  }

  pub fn optional_list(name: impl Into<String>, scalar_type: ScalarType) -> Self {
    Self {
      name: name.into(),
      scalar_type,
      shape: ParameterShape::List,
      required: false,
    }
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterShape {
  #[default]
  Scalar,
  List,
}

impl ParameterShape {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Scalar => "scalar",
      Self::List => "list",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationDefinition {
  pub name: String,
  pub from: String,
  pub to: String,
  #[serde(default)]
  pub cardinality: RelationCardinality,
  #[serde(default)]
  pub required: bool,
  #[serde(default)]
  pub selection: Option<RelationSelection>,
  pub on: Expression,
}

impl RelationDefinition {
  pub fn new(
    name: impl Into<String>,
    from: impl Into<String>,
    to: impl Into<String>,
    on: Expression,
  ) -> Self {
    Self {
      name: name.into(),
      from: from.into(),
      to: to.into(),
      cardinality: RelationCardinality::One,
      required: false,
      selection: None,
      on,
    }
  }

  pub fn required(mut self) -> Self {
    self.required = true;
    self
  }

  pub fn many(mut self) -> Self {
    self.cardinality = RelationCardinality::Many;
    self
  }

  pub fn first_by(mut self, order_by: impl IntoIterator<Item = OrderByDefinition>) -> Self {
    self.selection = Some(RelationSelection::FirstBy {
      order_by: order_by.into_iter().collect(),
    });
    self
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationCardinality {
  #[default]
  One,
  Many,
}

impl RelationCardinality {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::One => "one",
      Self::Many => "many",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RelationSelection {
  FirstBy {
    #[serde(rename = "orderBy")]
    order_by: Vec<OrderByDefinition>,
  },
}

impl RelationSelection {
  pub fn order_by(&self) -> &[OrderByDefinition] {
    match self {
      Self::FirstBy { order_by } => order_by,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintDefinition {
  pub name: String,
  #[serde(default)]
  pub when: ConstraintCondition,
  pub predicate: Expression,
}

impl ConstraintDefinition {
  pub fn always(name: impl Into<String>, predicate: Expression) -> Self {
    Self {
      name: name.into(),
      when: ConstraintCondition::Always,
      predicate,
    }
  }

  pub fn when_parameter(
    name: impl Into<String>,
    parameter: impl Into<String>,
    predicate: Expression,
  ) -> Self {
    Self {
      name: name.into(),
      when: ConstraintCondition::ParameterPresent {
        parameter: parameter.into(),
      },
      predicate,
    }
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ConstraintCondition {
  #[default]
  Always,
  ParameterPresent {
    parameter: String,
  },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionDefinition {
  #[serde(default)]
  pub fields: Vec<ProjectionFieldDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionFieldDefinition {
  pub path: Vec<String>,
  pub expression: Expression,
  #[serde(default, skip_serializing_if = "ProjectionFieldRole::is_value")]
  pub role: ProjectionFieldRole,
  #[serde(default)]
  pub selected_by_default: bool,
}

impl ProjectionFieldDefinition {
  pub fn new(path: Vec<String>, expression: Expression) -> Self {
    Self {
      path,
      expression,
      role: ProjectionFieldRole::Value,
      selected_by_default: false,
    }
  }

  pub fn dimension(mut self) -> Self {
    self.role = ProjectionFieldRole::Dimension;
    self
  }

  pub fn measure(mut self) -> Self {
    self.role = ProjectionFieldRole::Measure;
    self
  }

  pub fn selected_by_default(mut self) -> Self {
    self.selected_by_default = true;
    self
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectionFieldRole {
  #[default]
  Value,
  Dimension,
  Measure,
}

impl ProjectionFieldRole {
  const fn is_value(&self) -> bool {
    matches!(self, Self::Value)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrderByDefinition {
  pub expression: Expression,
  pub direction: OrderDirection,
  #[serde(default)]
  pub nulls: Option<NullsOrder>,
}

impl OrderByDefinition {
  pub fn asc(expression: Expression) -> Self {
    Self {
      expression,
      direction: OrderDirection::Asc,
      nulls: None,
    }
  }

  pub fn desc(expression: Expression) -> Self {
    Self {
      expression,
      direction: OrderDirection::Desc,
      nulls: None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderDirection {
  Asc,
  Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NullsOrder {
  First,
  Last,
}

fn default_true() -> bool {
  true
}
