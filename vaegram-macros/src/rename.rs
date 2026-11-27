//! Command names: the rename rules and the checks a name must pass.

use syn::LitStr;

/// Longest command name, in characters, as the runtime reads commands.
const MAX_NAME: usize = 32;

/// How a variant's name becomes its command name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameRule {
    /// `SetAge` is `setage`.
    Lowercase,
    /// `SetAge` is `SETAGE`.
    Uppercase,
    /// `set_age` is `SetAge`.
    PascalCase,
    /// `SetAge` is `setAge`.
    CamelCase,
    /// `SetAge` is `set_age`. The default.
    SnakeCase,
    /// `SetAge` is `SET_AGE`.
    ScreamingSnakeCase,
    /// The variant's name as written.
    Identity,
}

impl RenameRule {
    pub(crate) fn parse(lit: &LitStr) -> syn::Result<RenameRule> {
        Ok(match lit.value().as_str() {
            "lowercase" => RenameRule::Lowercase,
            "UPPERCASE" => RenameRule::Uppercase,
            "PascalCase" => RenameRule::PascalCase,
            "camelCase" => RenameRule::CamelCase,
            "snake_case" => RenameRule::SnakeCase,
            "SCREAMING_SNAKE_CASE" => RenameRule::ScreamingSnakeCase,
            "identity" => RenameRule::Identity,
            "kebab-case" | "SCREAMING-KEBAB-CASE" => {
                return Err(syn::Error::new(
                    lit.span(),
                    "a command name can't contain `-`, so kebab rules can't name commands; use \"snake_case\"",
                ));
            }
            other => {
                return Err(syn::Error::new(
                    lit.span(),
                    format!(
                        "unknown rename_rule `{other}`; expected \"snake_case\", \"lowercase\", \"UPPERCASE\", \
                         \"PascalCase\", \"camelCase\", \"SCREAMING_SNAKE_CASE\" or \"identity\""
                    ),
                ));
            }
        })
    }

    /// The command name of the variant `ident`.
    pub(crate) fn apply(self, ident: &str) -> String {
        match self {
            RenameRule::Lowercase => ident.to_lowercase(),
            RenameRule::Uppercase => ident.to_uppercase(),
            RenameRule::Identity => ident.to_owned(),
            RenameRule::SnakeCase => join(ident, "_", str::to_lowercase),
            RenameRule::ScreamingSnakeCase => join(ident, "_", str::to_uppercase),
            RenameRule::PascalCase => join(ident, "", capitalize),
            RenameRule::CamelCase => {
                let mut words = words(ident).into_iter();
                let mut out = words.next().map(str::to_lowercase).unwrap_or_default();
                out.extend(words.map(capitalize));
                out
            }
        }
    }
}

fn join(ident: &str, separator: &str, case: fn(&str) -> String) -> String {
    words(ident).into_iter().map(case).collect::<Vec<_>>().join(separator)
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
        None => String::new(),
    }
}

/// Splits an identifier into words. A word ends at `_`, before an uppercase letter that follows a lowercase letter
/// or a digit, and before the last uppercase letter of a run that a lowercase letter follows. So `HTTPStatus` is
/// `HTTP` and `Status`, and digits stay with the word before them: `Top10` is one word. These splits name commands,
/// so changing them renames commands in every bot: keep them.
fn words(ident: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = ident.char_indices().collect();
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &(at, c)) in chars.iter().enumerate() {
        if c == '_' {
            if let Some(s) = start.take() {
                words.push(&ident[s..at]);
            }
            continue;
        }
        if let Some(s) = start {
            let prev = chars[i - 1].1;
            let next_lower = chars.get(i + 1).is_some_and(|&(_, n)| n.is_lowercase());
            let boundary =
                c.is_uppercase() && (prev.is_lowercase() || prev.is_numeric() || (prev.is_uppercase() && next_lower));
            if boundary {
                words.push(&ident[s..at]);
                start = Some(at);
            }
        } else {
            start = Some(at);
        }
    }
    if let Some(s) = start {
        words.push(&ident[s..]);
    }
    words
}

/// Whether the runtime can read `name` as a command: 1 to 32 letters, digits or underscores.
pub(crate) fn is_command_name(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.chars().count()) && name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Whether Telegram takes `name` in a bot's command menu: 1 to 32 lowercase English letters, digits or underscores.
pub(crate) fn is_menu_name(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// `name` lowercased char by char, the way the command filters compare names without case.
pub(crate) fn fold(name: &str) -> String {
    name.chars().flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_splits_words_the_documented_way() {
        let snake = |ident| RenameRule::SnakeCase.apply(ident);
        assert_eq!(snake("Start"), "start");
        assert_eq!(snake("SetAge"), "set_age");
        assert_eq!(snake("HTTPStatus"), "http_status");
        assert_eq!(snake("Top10"), "top10");
        assert_eq!(snake("Get2Fa"), "get2_fa");
        assert_eq!(snake("ABC"), "abc");
        assert_eq!(snake("already_snake"), "already_snake");
        assert_eq!(snake("Ban_User"), "ban_user");
    }

    #[test]
    fn every_rule_names_set_age() {
        let all = [
            (RenameRule::Lowercase, "setage"),
            (RenameRule::Uppercase, "SETAGE"),
            (RenameRule::PascalCase, "SetAge"),
            (RenameRule::CamelCase, "setAge"),
            (RenameRule::SnakeCase, "set_age"),
            (RenameRule::ScreamingSnakeCase, "SET_AGE"),
            (RenameRule::Identity, "SetAge"),
        ];
        for (rule, name) in all {
            assert_eq!(rule.apply("SetAge"), name, "{rule:?}");
        }
        assert_eq!(RenameRule::PascalCase.apply("set_age"), "SetAge");
        assert_eq!(RenameRule::CamelCase.apply("HTTPStatus"), "httpStatus");
    }

    #[test]
    fn names_the_runtime_and_the_menu_take() {
        assert!(is_command_name("start") && is_command_name("Старт") && is_command_name(&"a".repeat(32)));
        assert!(!is_command_name("") && !is_command_name("set-age") && !is_command_name(&"a".repeat(33)));
        assert!(is_menu_name("set_age2"));
        assert!(!is_menu_name("SetAge") && !is_menu_name("Старт") && !is_menu_name(&"a".repeat(33)));
        assert_eq!(fold("ÄBC_d"), "äbc_d");
    }
}
