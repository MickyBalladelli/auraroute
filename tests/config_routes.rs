use auraroute::config::AppConfig;

#[test]
fn selects_fast_and_deep_routes_by_complexity() -> Result<(), Box<dyn std::error::Error>> {
    let raw = r#"
listen = "127.0.0.1:8080"

[[models]]
name = "fast"
upstream = "http://localhost:11434/v1/chat/completions"
max_complexity = 2

[[models]]
name = "deep"
upstream = "http://localhost:11435/v1/chat/completions"
min_complexity = 3
"#;

    let config: AppConfig = toml::from_str(raw)?;

    let fast = config
        .select_model(2)
        .ok_or("complexity 2 should select a model")?;
    assert_eq!(fast.name, "fast");
    assert_eq!(fast.upstream, "http://localhost:11434/v1/chat/completions");

    let deep = config
        .select_model(4)
        .ok_or("complexity 4 should select a model")?;
    assert_eq!(deep.name, "deep");
    assert_eq!(deep.upstream, "http://localhost:11435/v1/chat/completions");

    Ok(())
}
