use auraroute::config::AppConfig;

/// Parse a TOML string and run validation, returning either the parsed config or an error string.
fn parse_and_validate(raw: &str) -> Result<AppConfig, String> {
    let config = toml::from_str::<AppConfig>(raw)
        .map_err(|e| format!("toml parse error: {e}"))?;
    config.validate().map_err(|e| e.to_string())?;
    Ok(config)
}

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
    assert_eq!(
        reasoning.upstream,
        "http://localhost:11436/v1/chat/completions"
    );

    Ok(())
}

#[test]
fn validation_rejects_empty_model_name() {
    let raw = r#"
listen = "127.0.0.1:8080"

[[models]]
name = ""
upstream = "http://localhost:11434/v1"
kind = "fast"
"#;

    let result = parse_and_validate(raw);
    assert!(result.is_err(), "expected validation error for empty model name");
    assert!(
        result.unwrap_err().contains("model name cannot be empty"),
        "wrong error message"
    );
}

#[test]
fn validation_rejects_empty_upstream() {
    let raw = r#"
listen = "127.0.0.1:8080"

[[models]]
name = "fast"
upstream = ""
kind = "fast"
"#;

    let result = parse_and_validate(raw);
    assert!(result.is_err(), "expected validation error for empty upstream");
    assert!(
        result.unwrap_err().contains("upstream cannot be empty"),
        "wrong error message"
    );
}

#[test]
fn validation_rejects_inverted_complexity_range() {
    let raw = r#"
listen = "127.0.0.1:8080"

[[models]]
name = "broken"
upstream = "http://localhost:11434/v1"
kind = "fast"
min_complexity = 4
max_complexity = 2
"#;

    let result = parse_and_validate(raw);
    assert!(
        result.is_err(),
        "expected validation error for inverted complexity range"
    );
    assert!(
        result.unwrap_err().contains("min_complexity cannot exceed max_complexity"),
        "wrong error message"
    );
}

#[test]
fn validation_rejects_empty_models_list() {
    let raw = r#"
listen = "127.0.0.1:8080"
models = []
"#;

    let result = parse_and_validate(raw);
    assert!(result.is_err(), "expected validation error for empty models list");
    assert!(
        result
            .unwrap_err()
            .contains("at least one local model route is required"),
        "wrong error message"
    );
}

#[test]
fn validation_rejects_empty_listen() {
    let raw = r#"
listen = ""

[[models]]
name = "fast"
upstream = "http://localhost:11434/v1"
kind = "fast"
"#;

    let result = parse_and_validate(raw);
    assert!(result.is_err(), "expected validation error for empty listen");
    assert!(
        result.unwrap_err().contains("listen address cannot be empty"),
        "wrong error message"
    );
}

#[test]
fn validation_rejects_missing_kind() {
    let raw = r#"
listen = "127.0.0.1:8080"

[[models]]
name = "anonymous"
upstream = "http://localhost:11434/v1"
"#;

    let result = parse_and_validate(raw);
    assert!(result.is_err(), "expected validation error for missing kind");
    assert!(
        result.unwrap_err().contains("must have an explicit 'kind'"),
        "wrong error message"
    );
}

#[test]
fn validation_rejects_unknown_field_in_config() {
    let raw = r#"
listen = "127.0.0.1:8080"
typo_field = "oops"

[[models]]
name = "fast"
upstream = "http://localhost:11434/v1"
kind = "fast"
"#;

    let result: Result<AppConfig, _> = toml::from_str(raw);
    assert!(
        result.is_err(),
        "expected parse error for unknown field in config"
    );
}