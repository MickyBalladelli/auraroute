use auraroute::config::AppConfig;

#[test]
fn selects_fast_and_deep_routes_by_complexity() -> Result<(), Box<dyn std::error::Error>> {
    let raw = r#"
listen = "127.0.0.1:8080"

[[models]]
name = "fast"
upstream = "http://localhost:11434/v1/chat/completions"
kind = "fast"
max_complexity = 2

[[models]]
name = "code"
upstream = "http://localhost:11435/v1/chat/completions"
kind = "code"

[[models]]
name = "reasoning"
upstream = "http://localhost:11436/v1/chat/completions"
kind = "reasoning"
min_complexity = 3
"#;

    let config: AppConfig = toml::from_str(raw)?;

    let fast = config
        .select_model(2, false)
        .ok_or("complexity 2 should select a model")?;
    assert_eq!(fast.name, "fast");
    assert_eq!(fast.upstream, "http://localhost:11434/v1/chat/completions");

    let code = config
        .select_model(2, true)
        .ok_or("code prompt should select a model")?;
    assert_eq!(code.name, "code");
    assert_eq!(code.upstream, "http://localhost:11435/v1/chat/completions");

    let reasoning = config
        .select_model(4, false)
        .ok_or("complexity 4 should select a model")?;
    assert_eq!(reasoning.name, "reasoning");
    assert_eq!(reasoning.upstream, "http://localhost:11436/v1/chat/completions");

    Ok(())
}
