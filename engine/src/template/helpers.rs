use chrono::Utc;
use fakeit::{bool_rand, contact, internet, misc, name};
use uuid::Uuid;

pub(crate) fn resolve_helper(helper_expression: &str) -> Option<String> {
    let mut parts = helper_expression.split_whitespace();
    let name = parts.next()?;

    match name {
        "uuid" => Some(Uuid::new_v4().to_string()),
        "email" => Some(contact::email()),
        "name" => Some(name::full()),
        "username" => Some(internet::username().to_lowercase()),
        "number" => {
            let min = parts.next().unwrap_or("0").parse::<i64>().unwrap_or(0);
            let max = parts.next().unwrap_or("100").parse::<i64>().unwrap_or(100);
            let (min, max) = if min <= max { (min, max) } else { (max, min) };
            if min == max {
                return Some(min.to_string());
            }
            let upper_exclusive = max.checked_add(1).unwrap_or(max);
            Some(misc::random::<i64>(min, upper_exclusive).to_string())
        }
        "date" => Some(Utc::now().date_naive().to_string()),
        "boolean" => Some(bool_rand::bool().to_string()),
        "cpf" => Some(generate_cpf()),
        _ => None,
    }
}

pub(crate) fn generate_cpf() -> String {
    let digits = misc::replace_with_numbers("###########".to_owned());
    let chars: Vec<char> = digits.chars().collect();
    format!(
        "{}{}{}.{}{}{}.{}{}{}-{}{}",
        chars[0],
        chars[1],
        chars[2],
        chars[3],
        chars[4],
        chars[5],
        chars[6],
        chars[7],
        chars[8],
        chars[9],
        chars[10]
    )
}
