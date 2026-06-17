use std::sync::LazyLock;

use regex::Regex;

static CODE_SYNTAX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\{|\}|=>|\bfn\b|\basync\b|\bawait\b|\bimpl\b|\btrait\b|::|->)").expect("code syntax regex must compile"));
static CODE_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)(```|~~~|\b(fn|async|impl|trait|struct|enum|use)\b)").expect("code block regex must compile"));
static ARCHITECTURE_KEYWORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(architect|architecture|refactor|bottleneck|deadlock|concurrency|distributed|scalability|race condition|contention)\b",
    )
    .expect("architecture keyword regex must compile")
});

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

pub fn looks_like_code(prompt: &str) -> bool {
    has_code_block(prompt) || has_dense_code_syntax(prompt)
}

fn has_code_block(prompt: &str) -> bool {
    CODE_BLOCK_RE.is_match(prompt)
}

fn has_dense_code_syntax(prompt: &str) -> bool {
    let char_count = prompt.chars().count();
    if char_count == 0 {
        return false;
    }

    let syntax_hits = CODE_SYNTAX_RE.find_iter(prompt).count();
    let syntax_density = syntax_hits as f32 / char_count as f32;

    syntax_hits >= 6 && syntax_density >= 0.015
}

fn has_architecture_keywords(prompt: &str) -> bool {
    ARCHITECTURE_KEYWORD_RE.is_match(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prompt_is_complexity_1() {
        assert_eq!(calculate_complexity("", 0), 1);
    }

    #[test]
    fn simple_prompt_is_complexity_1() {
        assert_eq!(calculate_complexity("hello world", 3), 1);
    }

    #[test]
    fn long_prompt_adds_2() {
        assert_eq!(calculate_complexity("word ".repeat(1501).trim(), 1501), 3);
    }

    #[test]
    fn code_syntax_adds_1() {
        let code = "fn main() { let x = 1; let y = 2; { fn foo() { async fn bar() { impl Trait for Type {} } } } }";
        assert_eq!(calculate_complexity(code, 20), 2);
    }

    #[test]
    fn architecture_keywords_add_1() {
        let prompt =
            "We need to architect a refactoring to fix the deadlock and bottleneck in the distributed system";
        assert_eq!(calculate_complexity(prompt, 20), 2);
    }

    #[test]
    fn combined_hits_stack_to_max_5() {
        let prompt = "architect refactor deadlock ".to_string()
            + &"fn main() { async fn bar() { impl Trait {} } }".repeat(20)
            + &"word ".repeat(1501);
        let score = calculate_complexity(&prompt, 1501);
        // base 1 + long 2 + code 1 + architecture 1 = 5, clamped to 5
        assert_eq!(score, 5);
    }

    #[test]
    fn code_block_detection() {
        assert!(looks_like_code("```rust\nfn main() {}\n```"));
        assert!(looks_like_code("```\nsome code\n```"));
    }

    #[test]
    fn plain_text_not_code() {
        assert!(!looks_like_code("hello, how are you today?"));
    }

    #[test]
    fn dense_rust_syntax_is_code() {
        assert!(looks_like_code("fn main() { let x = 1; { let y = 2; impl Foo { fn bar() -> u8 { 1 } } } }"));
    }

    #[test]
    fn architecture_keyword_detection() {
        assert!(has_architecture_keywords("We need to architect a new solution"));
        assert!(has_architecture_keywords("There is a bottleneck in production"));
        assert!(!has_architecture_keywords("regular text without keywords"));
    }
}