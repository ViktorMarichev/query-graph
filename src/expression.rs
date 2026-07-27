use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum LiteralValue {
  Null,
  Boolean(bool),
  Integer(i64),
  Decimal(String),
  String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
  Function {
    name: String,
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

  pub fn for_each_field<'a>(&'a self, visitor: &mut impl FnMut(&'a str, &'a str)) {
    match self {
      Self::Field { source, field } => visitor(source, field),
      Self::Parameter { .. } | Self::Literal { .. } => {}
      Self::Eq { left, right }
      | Self::NotEq { left, right }
      | Self::LessThan { left, right }
      | Self::LessThanOrEqual { left, right }
      | Self::GreaterThan { left, right }
      | Self::GreaterThanOrEqual { left, right } => {
        left.for_each_field(visitor);
        right.for_each_field(visitor);
      }
      Self::Like {
        expression,
        pattern,
      } => {
        expression.for_each_field(visitor);
        pattern.for_each_field(visitor);
      }
      Self::In { expression, values } => {
        expression.for_each_field(visitor);
        for value in values {
          value.for_each_field(visitor);
        }
      }
      Self::And { expressions } | Self::Or { expressions } => {
        for expression in expressions {
          expression.for_each_field(visitor);
        }
      }
      Self::Not { expression } | Self::IsNull { expression } | Self::IsNotNull { expression } => {
        expression.for_each_field(visitor)
      }
      Self::Function { arguments, .. } => {
        for argument in arguments {
          argument.for_each_field(visitor);
        }
      }
    }
  }
}
