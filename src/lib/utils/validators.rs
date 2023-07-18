
pub fn is_integer_str(str_ref: &str) -> bool {
  str_ref.chars().map(|c| c.is_numeric()).collect::<Vec<bool>>().contains(&false) == false
}

pub fn is_decimal_str(str_ref: &str) -> bool {
  if str_ref.contains(".") {
    let parts = str_ref.split(".").map(|s| s).collect::<Vec<&str>>();
    if parts.len() == 2 {
      is_integer_str(parts[0]) && is_integer_str(parts[1])
    } else {
      false
    }
  } else {
    is_integer_str(str_ref)
  }
}
