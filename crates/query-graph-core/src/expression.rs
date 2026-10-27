use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
  tag = "kind",
  content = "value",
  rename_all = "camelCase",
  deny_unknown_fields
)]
pub enum LiteralValue {
  Null,
  Boolean(bool),
  Integer(i64),
  Decimal(String),
  String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticFunction {
  Lower,
  Upper,
  Coalesce,
  Concat,
  Divide,
  Truncate,
  Modulo,
}

impl SemanticFunction {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Lower => "lower",
      Self::Upper => "upper",
      Self::Coalesce => "coalesce",
      Self::Concat => "concat",
      Self::Divide => "divide",
      Self::Truncate => "truncate",
      Self::Modulo => "modulo",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AggregateFunction {
  Count,
  CountDistinct,
  Sum,
  Average,
  Minimum,
  Maximum,
}

impl AggregateFunction {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Count => "count",
      Self::CountDistinct => "countDistinct",
      Self::Sum => "sum",
      Self::Average => "average",
      Self::Minimum => "minimum",
      Self::Maximum => "maximum",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Expression {
  Field {
    source: String,
    field: String,
  },
  Parameter {
    name: String,
  },
  Literal {
    value: LiteralValue,
  },
  Eq {
    left: Box<Expression>,
    right: Box<Expression>,
  },
  NotEq {
    left: Box<Expression>,
    right: Box<Expression>,
  },
  LessThan {
    left: Box<Expression>,
    right: Box<Expression>,
  },
  LessThanOrEqual {
    left: Box<Expression>,
    right: Box<Expression>,
  },
  GreaterThan {
    left: Box<Expression>,
    right: Box<Expression>,
  },
  GreaterThanOrEqual {
    left: Box<Expression>,
    right: Box<Expression>,
  },
  Like {
    expression: Box<Expression>,
    pattern: Box<Expression>,
  },
  In {
    expression: Box<Expression>,
    values: Vec<Expression>,
  },
  InParameter {
    expression: Box<Expression>,
    parameter: String,
  },
  AllGroups {
    parameter: String,
    scope: String,
    predicate: Box<Expression>,
  },
  GroupKey {
    scope: String,
  },
  InGroupValues {
    expression: Box<Expression>,
    scope: String,
  },
  And {
    expressions: Vec<Expression>,
  },
  Or {
    expressions: Vec<Expression>,
  },
  Not {
    expression: Box<Expression>,
  },
  IsNull {
    expression: Box<Expression>,
  },
  IsNotNull {
    expression: Box<Expression>,
  },
  Exists {
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(default)]
    predicate: Option<Box<Expression>>,
  },
  Function {
    name: SemanticFunction,
    arguments: Vec<Expression>,
  },
  Aggregate {
    function: AggregateFunction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expression: Option<Box<Expression>>,
  },
}

impl Expression {
  pub fn field(source: impl Into<String>, field: impl Into<String>) -> Self {
    Self::Field {
      source: source.into(),
      field: field.into(),
    }
  }

  pub fn parameter(name: impl Into<String>) -> Self {
    Self::Parameter { name: name.into() }
  }

  pub fn literal(value: LiteralValue) -> Self {
    Self::Literal { value }
  }

  pub fn eq(left: Expression, right: Expression) -> Self {
    Self::Eq {
      left: Box::new(left),
      right: Box::new(right),
    }
  }

  pub fn and(expressions: impl IntoIterator<Item = Expression>) -> Self {
    Self::And {
      expressions: expressions.into_iter().collect(),
    }
  }

  pub fn or(expressions: impl IntoIterator<Item = Expression>) -> Self {
    Self::Or {
      expressions: expressions.into_iter().collect(),
    }
  }

  pub fn exists(source: impl Into<String>) -> Self {
    Self::Exists {
      source: source.into(),
      from: None,
      predicate: None,
    }
  }

  pub fn in_parameter(expression: Expression, parameter: impl Into<String>) -> Self {
    Self::InParameter {
      expression: Box::new(expression),
      parameter: parameter.into(),
    }
  }

  pub fn all_groups(
    parameter: impl Into<String>,
    scope: impl Into<String>,
    predicate: Expression,
  ) -> Self {
    Self::AllGroups {
      parameter: parameter.into(),
      scope: scope.into(),
      predicate: Box::new(predicate),
    }
  }

  pub fn group_key(scope: impl Into<String>) -> Self {
    Self::GroupKey {
      scope: scope.into(),
    }
  }

  pub fn in_group_values(expression: Expression, scope: impl Into<String>) -> Self {
    Self::InGroupValues {
      expression: Box::new(expression),
      scope: scope.into(),
    }
  }

  pub fn exists_where(source: impl Into<String>, predicate: Expression) -> Self {
    Self::Exists {
      source: source.into(),
      from: None,
      predicate: Some(Box::new(predicate)),
    }
  }

  pub fn exists_from(source: impl Into<String>, from: impl Into<String>) -> Self {
    Self::Exists {
      source: source.into(),
      from: Some(from.into()),
      predicate: None,
    }
  }

  pub fn exists_from_where(
    source: impl Into<String>,
    from: impl Into<String>,
    predicate: Expression,
  ) -> Self {
    Self::Exists {
      source: source.into(),
      from: Some(from.into()),
      predicate: Some(Box::new(predicate)),
    }
  }

  pub fn aggregate(function: AggregateFunction, expression: Option<Expression>) -> Self {
    Self::Aggregate {
      function,
      expression: expression.map(Box::new),
    }
  }

  pub fn count() -> Self {
    Self::aggregate(AggregateFunction::Count, None)
  }

  pub fn count_of(expression: Expression) -> Self {
    Self::aggregate(AggregateFunction::Count, Some(expression))
  }

  pub fn count_distinct(expression: Expression) -> Self {
    Self::aggregate(AggregateFunction::CountDistinct, Some(expression))
  }

  pub fn sum(expression: Expression) -> Self {
    Self::aggregate(AggregateFunction::Sum, Some(expression))
  }

  pub fn average(expression: Expression) -> Self {
    Self::aggregate(AggregateFunction::Average, Some(expression))
  }

  pub fn minimum(expression: Expression) -> Self {
    Self::aggregate(AggregateFunction::Minimum, Some(expression))
  }

  pub fn maximum(expression: Expression) -> Self {
    Self::aggregate(AggregateFunction::Maximum, Some(expression))
  }

  pub fn contains_aggregate(&self) -> bool {
    let mut contains_aggregate = false;
    self.walk(&mut |expression| {
      if matches!(expression, Self::Aggregate { .. }) {
        contains_aggregate = true;
        return false;
      }
      true
    });
    contains_aggregate
  }

  pub fn for_each_field<'a>(&'a self, visitor: &mut impl FnMut(&'a str, &'a str)) {
    self.walk(&mut |expression| {
      if let Self::Field { source, field } = expression {
        visitor(source, field);
      }
      true
    });
  }

  pub(crate) fn for_each_exists_source<'a>(
    &'a self,
    visitor: &mut impl FnMut(&'a str, Option<&'a str>),
  ) {
    self.walk(&mut |expression| {
      if let Self::Exists { source, from, .. } = expression {
        visitor(source, from.as_deref());
      }
      true
    });
  }

  pub(crate) fn walk<'a>(&'a self, visitor: &mut impl FnMut(&'a Self) -> bool) {
    if !visitor(self) {
      return;
    }
    self.for_each_child(&mut |child| child.walk(visitor));
  }

  pub(crate) fn for_each_child<'a>(&'a self, visitor: &mut impl FnMut(&'a Self)) {
    match self {
      Self::Field { .. }
      | Self::Parameter { .. }
      | Self::Literal { .. }
      | Self::GroupKey { .. } => {}
      Self::Eq { left, right }
      | Self::NotEq { left, right }
      | Self::LessThan { left, right }
      | Self::LessThanOrEqual { left, right }
      | Self::GreaterThan { left, right }
      | Self::GreaterThanOrEqual { left, right } => {
        visitor(left);
        visitor(right);
      }
      Self::Like {
        expression,
        pattern,
      } => {
        visitor(expression);
        visitor(pattern);
      }
      Self::In { expression, values } => {
        visitor(expression);
        for value in values {
          visitor(value);
        }
      }
      Self::InParameter { expression, .. } | Self::InGroupValues { expression, .. } => {
        visitor(expression);
      }
      Self::AllGroups { predicate, .. } => visitor(predicate),
      Self::And { expressions } | Self::Or { expressions } => {
        for expression in expressions {
          visitor(expression);
        }
      }
      Self::Not { expression } | Self::IsNull { expression } | Self::IsNotNull { expression } => {
        visitor(expression);
      }
      Self::Exists { predicate, .. } => {
        if let Some(predicate) = predicate {
          visitor(predicate);
        }
      }
      Self::Function { arguments, .. } => {
        for argument in arguments {
          visitor(argument);
        }
      }
      Self::Aggregate { expression, .. } => {
        if let Some(expression) = expression {
          visitor(expression);
        }
      }
    }
  }
}
