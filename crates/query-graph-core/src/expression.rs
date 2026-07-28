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
}

impl SemanticFunction {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Lower => "lower",
      Self::Upper => "upper",
      Self::Coalesce => "coalesce",
      Self::Concat => "concat",
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
    #[serde(default)]
    predicate: Option<Box<Expression>>,
  },
  Function {
    name: SemanticFunction,
    arguments: Vec<Expression>,
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
      predicate: None,
    }
  }

  pub fn exists_where(source: impl Into<String>, predicate: Expression) -> Self {
    Self::Exists {
      source: source.into(),
      predicate: Some(Box::new(predicate)),
    }
  }

  pub fn for_each_field<'a>(&'a self, visitor: &mut impl FnMut(&'a str, &'a str)) {
    self.walk(&mut |expression| {
      if let Self::Field { source, field } = expression {
        visitor(source, field);
      }
      true
    });
  }

  pub(crate) fn for_each_outer_field<'a>(&'a self, visitor: &mut impl FnMut(&'a str, &'a str)) {
    self.walk(&mut |expression| {
      if let Self::Field { source, field } = expression {
        visitor(source, field);
      }
      !matches!(expression, Self::Exists { .. })
    });
  }

  pub(crate) fn for_each_exists_source<'a>(&'a self, visitor: &mut impl FnMut(&'a str)) {
    self.walk(&mut |expression| {
      if let Self::Exists { source, .. } = expression {
        visitor(source);
      }
      true
    });
  }

  pub(crate) fn for_each_parameter<'a>(&'a self, visitor: &mut impl FnMut(&'a str)) {
    self.walk(&mut |expression| {
      if let Self::Parameter { name } = expression {
        visitor(name);
      }
      true
    });
  }

  fn walk<'a>(&'a self, visitor: &mut impl FnMut(&'a Self) -> bool) {
    if !visitor(self) {
      return;
    }

    match self {
      Self::Field { .. } | Self::Parameter { .. } | Self::Literal { .. } => {}
      Self::Eq { left, right }
      | Self::NotEq { left, right }
      | Self::LessThan { left, right }
      | Self::LessThanOrEqual { left, right }
      | Self::GreaterThan { left, right }
      | Self::GreaterThanOrEqual { left, right } => {
        left.walk(visitor);
        right.walk(visitor);
      }
      Self::Like {
        expression,
        pattern,
      } => {
        expression.walk(visitor);
        pattern.walk(visitor);
      }
      Self::In { expression, values } => {
        expression.walk(visitor);
        for value in values {
          value.walk(visitor);
        }
      }
      Self::And { expressions } | Self::Or { expressions } => {
        for expression in expressions {
          expression.walk(visitor);
        }
      }
      Self::Not { expression } | Self::IsNull { expression } | Self::IsNotNull { expression } => {
        expression.walk(visitor);
      }
      Self::Exists { predicate, .. } => {
        if let Some(predicate) = predicate {
          predicate.walk(visitor);
        }
      }
      Self::Function { arguments, .. } => {
        for argument in arguments {
          argument.walk(visitor);
        }
      }
    }
  }
}
