pub(crate) fn is_decimal_text(value: &str) -> bool {
  let value = value.strip_prefix('-').unwrap_or(value);
  let mut has_digit = false;
  let mut has_decimal_point = false;

  for character in value.chars() {
    if character.is_ascii_digit() {
      has_digit = true;
    } else if character == '.' && !has_decimal_point {
      has_decimal_point = true;
    } else {
      return false;
    }
  }

  has_digit
}

#[cfg(test)]
mod tests {
  use super::is_decimal_text;

  #[test]
  fn accepts_finite_decimal_notation_only() {
    for value in ["0", "-10", "1.25", ".5", "5."] {
      assert!(is_decimal_text(value), "{value}");
    }

    for value in ["", "-", ".", "NaN", "inf", "1e3", "+1", "1.2.3"] {
      assert!(!is_decimal_text(value), "{value}");
    }
  }
}
