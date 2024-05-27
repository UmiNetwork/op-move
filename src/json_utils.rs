pub fn get_field(x: &serde_json::Value, name: &str) -> serde_json::Value {
    x.as_object()
        .and_then(|o| o.get(name))
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}
