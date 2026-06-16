use std::sync::OnceLock;

use regex::Regex;

static CODE_SYNTAX_RE: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();
static ARCHITECTURE_KEYWORD_RE: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();

pub fn calculate_complexity(prompt: &str, token_count: usize) -> u8 {
    let mut score = 1u8;

    if token_count > 1_500 {
        score = score.saturating_add(2);
    }

    if has_dense_code_syntax(prompt) {
        score = score.saturating_add(1);
    }

    if has_architecture_keywords(prompt) {
        score = score.saturating_add(1);
    }

    score.clamp(1, 5)
}

fn has_dense_code_syntax(prompt: &str) -> bool {
    let char_count = prompt.chars().count();
    if char_count == 0 {
        return false;
    }

    let Some(regex) = compiled_code_syntax_regex() else {
        return false;
    };

    let syntax_hits = regex.find_iter(prompt).count();
    let syntax_density = syntax_hits as f32 / char_count as f32;

    syntax_hits >= 6 && syntax_density >= 0.015
}

fn has_architecture_keywords(prompt: &str) -> bool {
    compiled_architecture_keyword_regex()
        .map(|regex| regex.is_match(prompt))
        .unwrap_or(false)
}

fn compiled_code_syntax_regex() -> Option<&'static Regex> {
    CODE_SYNTAX_RE
        .get_or_init(|| Regex::new(r"(\{|\}|=>|\bfn\b|\basync\b|\bawait\b|\bimpl\b|\btrait\b|::|->)"))
        .as_ref()
        .ok()
}

fn compiled_architecture_keyword_regex() -> Option<&'static Regex> {
    ARCHITECTURE_KEYWORD_RE
        .get_or_init(|| {
            Regex::new(
                r"(?i)\b(architect|architecture|refactor|bottleneck|deadlock|concurrency|distributed|scalability|race condition|contention)\b",
            )
        })
        .as_ref()
        .ok()
}
