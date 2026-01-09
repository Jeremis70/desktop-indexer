use serde::Serialize;

pub fn print_json<T: Serialize>(value: &T) {
    let s = serde_json::to_string_pretty(value).unwrap();
    println!("{s}");
}
