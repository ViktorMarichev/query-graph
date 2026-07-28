use serde::{Deserialize, Serialize};

use crate::{LiteralValue, ScalarType, SemanticFunction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressionType {
  pub scalar_type: ScalarType,
  pub nullable: bool,
}

impl ExpressionType {
  pub const fn new(scalar_type: ScalarType, nullable: bool) -> Self {
    Self {
      scalar_type,
      nullable,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InferredType {
  pub(crate) scalar_type: Option<ScalarType>,
  pub(crate) nullable: bool,
}

impl InferredType {
  pub(crate) const fn scalar(scalar_type: ScalarType, nullable: bool) -> Self {
    Self {
      scalar_type: Some(scalar_type),
      nullable,
    }
  }

  pub(crate) const fn parameter(scalar_type: ScalarType) -> Self {
    Self::scalar(scalar_type, false)
  }

  pub(crate) const fn literal(value: &LiteralValue) -> Self {
    match value {
      LiteralValue::Null => Self {
        scalar_type: None,
        nullable: true,
      },
      LiteralValue::Boolean(_) => Self::scalar(ScalarType::Boolean, false),
      LiteralValue::Integer(_) => Self::scalar(ScalarType::Int64, false),
      LiteralValue::Decimal(_) => Self::scalar(ScalarType::Decimal, false),
      LiteralValue::String(_) => Self::scalar(ScalarType::String, false),
    }
  }

  fn describe(self) -> String {
    self
      .scalar_type
      .map_or("null", ScalarType::as_str)
      .to_owned()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeSystemErrorKind {
  IncompatibleTypes,
  InvalidType,
  InvalidFunctionArity,
  UnresolvedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeSystemError {
  pub(crate) kind: TypeSystemErrorKind,
  pub(crate) message: String,
}

impl TypeSystemError {
  fn new(kind: TypeSystemErrorKind, message: impl Into<String>) -> Self {
    Self {
      kind,
      message: message.into(),
    }
  }
}

pub(crate) fn infer_equality(
  left: InferredType,
  right: InferredType,
) -> Result<InferredType, TypeSystemError> {
  if left.scalar_type.is_none() || right.scalar_type.is_none() {
    return Ok(InferredType::scalar(ScalarType::Boolean, false));
  }

  let common = common_scalar_type(left.scalar_type, right.scalar_type)
    .map_err(|()| incompatible_types("equality comparison", left, right))?;
  let Some(common) = common else {
    return Ok(InferredType::scalar(ScalarType::Boolean, false));
  };

  if common == ScalarType::Json {
    return Err(TypeSystemError::new(
      TypeSystemErrorKind::InvalidType,
      "equality comparison is not defined for json values",
    ));
  }

  Ok(InferredType::scalar(
    ScalarType::Boolean,
    left.nullable || right.nullable,
  ))
}

pub(crate) fn infer_ordering_comparison(
  left: InferredType,
  right: InferredType,
) -> Result<InferredType, TypeSystemError> {
  let common = common_scalar_type(left.scalar_type, right.scalar_type)
    .map_err(|()| incompatible_types("ordering comparison", left, right))?;
  let Some(common) = common else {
    return Err(unresolved_type("ordering comparison"));
  };

  if !is_orderable(common) {
    return Err(TypeSystemError::new(
      TypeSystemErrorKind::InvalidType,
      format!(
        "ordering comparison requires orderable values, received {} and {}",
        left.describe(),
        right.describe()
      ),
    ));
  }

  Ok(InferredType::scalar(
    ScalarType::Boolean,
    left.nullable || right.nullable,
  ))
}

pub(crate) fn infer_like(
  expression: InferredType,
  pattern: InferredType,
) -> Result<InferredType, TypeSystemError> {
  require_string_or_null(expression, "LIKE expression")?;
  require_string_or_null(pattern, "LIKE pattern")?;

  if expression.scalar_type.is_none() && pattern.scalar_type.is_none() {
    return Err(unresolved_type("LIKE expression"));
  }

  Ok(InferredType::scalar(
    ScalarType::Boolean,
    expression.nullable || pattern.nullable,
  ))
}

pub(crate) fn infer_in(
  expression: InferredType,
  values: &[InferredType],
) -> Result<InferredType, TypeSystemError> {
  let mut nullable = expression.nullable;
  for value in values {
    infer_equality(expression, *value)
      .map_err(|_| incompatible_types("IN expression", expression, *value))?;
    nullable |= value.nullable;
  }

  Ok(InferredType::scalar(ScalarType::Boolean, nullable))
}

pub(crate) fn infer_logical(expressions: &[InferredType]) -> Result<InferredType, TypeSystemError> {
  let mut nullable = false;
  for expression in expressions {
    require_boolean_or_null(*expression, "logical operand")?;
    nullable |= expression.nullable;
  }

  Ok(InferredType::scalar(ScalarType::Boolean, nullable))
}

pub(crate) fn infer_not(expression: InferredType) -> Result<InferredType, TypeSystemError> {
  require_boolean_or_null(expression, "NOT operand")?;
  Ok(InferredType::scalar(
    ScalarType::Boolean,
    expression.nullable,
  ))
}

pub(crate) fn infer_null_test(_expression: InferredType) -> Result<InferredType, TypeSystemError> {
  Ok(InferredType::scalar(ScalarType::Boolean, false))
}

pub(crate) fn infer_function(
  function: SemanticFunction,
  arguments: &[InferredType],
) -> Result<InferredType, TypeSystemError> {
  match function {
    SemanticFunction::Lower | SemanticFunction::Upper => {
      infer_unary_string_function(function.as_str(), arguments)
    }
    SemanticFunction::Coalesce => infer_coalesce(arguments),
    SemanticFunction::Concat => infer_concat(arguments),
  }
}

pub(crate) fn require_predicate(expression: InferredType) -> Result<(), TypeSystemError> {
  if expression.scalar_type == Some(ScalarType::Boolean) {
    Ok(())
  } else {
    Err(TypeSystemError::new(
      TypeSystemErrorKind::InvalidType,
      format!(
        "predicate must be a boolean expression, received {}",
        expression.describe()
      ),
    ))
  }
}

pub(crate) fn require_orderable(expression: InferredType) -> Result<(), TypeSystemError> {
  let Some(scalar_type) = expression.scalar_type else {
    return Err(unresolved_type("order expression"));
  };

  if is_orderable(scalar_type) {
    Ok(())
  } else {
    Err(TypeSystemError::new(
      TypeSystemErrorKind::InvalidType,
      format!(
        "order expression must be orderable, received {}",
        expression.describe()
      ),
    ))
  }
}

pub(crate) fn resolve_expression_type(
  expression: InferredType,
) -> Result<ExpressionType, TypeSystemError> {
  expression
    .scalar_type
    .map(|scalar_type| ExpressionType::new(scalar_type, expression.nullable))
    .ok_or_else(|| unresolved_type("projection expression"))
}

fn infer_unary_string_function(
  name: &str,
  arguments: &[InferredType],
) -> Result<InferredType, TypeSystemError> {
  require_arity(name, arguments.len(), 1, 1)?;
  let argument = arguments[0];
  require_string_or_null(argument, &format!("{name} argument"))?;
  Ok(InferredType::scalar(ScalarType::String, argument.nullable))
}

fn infer_coalesce(arguments: &[InferredType]) -> Result<InferredType, TypeSystemError> {
  require_arity("coalesce", arguments.len(), 2, usize::MAX)?;

  let mut common = None;
  for argument in arguments {
    common = common_scalar_type(common, argument.scalar_type).map_err(|()| {
      TypeSystemError::new(
        TypeSystemErrorKind::IncompatibleTypes,
        format!(
          "coalesce arguments have incompatible types: {}",
          arguments
            .iter()
            .map(|argument| argument.describe())
            .collect::<Vec<_>>()
            .join(", ")
        ),
      )
    })?;
  }

  let Some(scalar_type) = common else {
    return Err(unresolved_type("coalesce result"));
  };

  Ok(InferredType::scalar(
    scalar_type,
    arguments.iter().all(|argument| argument.nullable),
  ))
}

fn infer_concat(arguments: &[InferredType]) -> Result<InferredType, TypeSystemError> {
  require_arity("concat", arguments.len(), 1, usize::MAX)?;
  for argument in arguments {
    require_string_or_null(*argument, "concat argument")?;
  }

  Ok(InferredType::scalar(
    ScalarType::String,
    arguments.iter().all(|argument| argument.nullable),
  ))
}

fn require_arity(
  name: &str,
  actual: usize,
  minimum: usize,
  maximum: usize,
) -> Result<(), TypeSystemError> {
  if actual >= minimum && actual <= maximum {
    return Ok(());
  }

  let expected = if minimum == maximum {
    minimum.to_string()
  } else {
    format!("at least {minimum}")
  };

  Err(TypeSystemError::new(
    TypeSystemErrorKind::InvalidFunctionArity,
    format!("semantic function {name:?} expects {expected} argument(s), received {actual}"),
  ))
}

fn require_string_or_null(value: InferredType, role: &str) -> Result<(), TypeSystemError> {
  if matches!(value.scalar_type, None | Some(ScalarType::String)) {
    Ok(())
  } else {
    Err(TypeSystemError::new(
      TypeSystemErrorKind::InvalidType,
      format!("{role} must be string, received {}", value.describe()),
    ))
  }
}

fn require_boolean_or_null(value: InferredType, role: &str) -> Result<(), TypeSystemError> {
  if matches!(value.scalar_type, None | Some(ScalarType::Boolean)) {
    Ok(())
  } else {
    Err(TypeSystemError::new(
      TypeSystemErrorKind::InvalidType,
      format!("{role} must be boolean, received {}", value.describe()),
    ))
  }
}

fn incompatible_types(operation: &str, left: InferredType, right: InferredType) -> TypeSystemError {
  TypeSystemError::new(
    TypeSystemErrorKind::IncompatibleTypes,
    format!(
      "{operation} cannot combine {} and {}",
      left.describe(),
      right.describe()
    ),
  )
}

fn unresolved_type(role: &str) -> TypeSystemError {
  TypeSystemError::new(
    TypeSystemErrorKind::UnresolvedType,
    format!("{role} has no inferable scalar type"),
  )
}

fn common_scalar_type(
  left: Option<ScalarType>,
  right: Option<ScalarType>,
) -> Result<Option<ScalarType>, ()> {
  match (left, right) {
    (None, value) | (value, None) => Ok(value),
    (Some(left), Some(right)) if left == right => Ok(Some(left)),
    (Some(left), Some(right)) if is_numeric(left) && is_numeric(right) => {
      Ok(Some(promote_numeric(left, right)))
    }
    (Some(ScalarType::Date), Some(ScalarType::DateTime))
    | (Some(ScalarType::DateTime), Some(ScalarType::Date)) => Ok(Some(ScalarType::DateTime)),
    _ => Err(()),
  }
}

fn promote_numeric(left: ScalarType, right: ScalarType) -> ScalarType {
  if left == ScalarType::Float64 || right == ScalarType::Float64 {
    ScalarType::Float64
  } else if left == ScalarType::Decimal || right == ScalarType::Decimal {
    ScalarType::Decimal
  } else if left == ScalarType::Int64 || right == ScalarType::Int64 {
    ScalarType::Int64
  } else {
    ScalarType::Int32
  }
}

fn is_numeric(scalar_type: ScalarType) -> bool {
  matches!(
    scalar_type,
    ScalarType::Int32 | ScalarType::Int64 | ScalarType::Float64 | ScalarType::Decimal
  )
}

fn is_orderable(scalar_type: ScalarType) -> bool {
  is_numeric(scalar_type)
    || matches!(
      scalar_type,
      ScalarType::String | ScalarType::Date | ScalarType::DateTime
    )
}
