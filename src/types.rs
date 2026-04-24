use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Fixed namespace UUID for deterministic v5 generation.
pub const NAMESPACE: Uuid = Uuid::from_bytes([
    0x7a, 0x65, 0x64, 0x2d, 0x72, 0x75, 0x6c, 0x65, 0x73, 0x2d, 0x73, 0x79, 0x6e, 0x63, 0x00, 0x01,
]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserPromptId(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltInPrompt {
    CommitMessage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PromptId {
    User { uuid: UserPromptId },
    BuiltIn(BuiltInPrompt),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptMetadata {
    pub id: PromptId,
    pub title: Option<String>,
    pub default: bool,
    pub saved_at: DateTime<Utc>,
}

pub fn prompt_id_for_filename(filename: &str) -> PromptId {
    let uuid = Uuid::new_v5(&NAMESPACE, filename.as_bytes());
    PromptId::User {
        uuid: UserPromptId(uuid),
    }
}

pub fn title_from_filename(filename: &str) -> String {
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    stem.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn title_to_filename(title: &str) -> String {
    let slug: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    format!("{}.md", result.trim_end_matches('-'))
}

pub fn is_managed(id: &PromptId, title: Option<&str>) -> bool {
    let PromptId::User { uuid } = id else {
        return false;
    };
    let Some(title) = title else { return false };
    let filename = title_to_filename(title);
    let expected = Uuid::new_v5(&NAMESPACE, filename.as_bytes());
    uuid.0 == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    // AIDEV-NOTE: Changing the NAMESPACE bytes detaches every previously-synced
    // rule on every user's machine. This test pins them to catch accidents.
    #[test]
    fn namespace_bytes_are_frozen() {
        assert_eq!(
            NAMESPACE.as_bytes(),
            &[
                0x7a, 0x65, 0x64, 0x2d, 0x72, 0x75, 0x6c, 0x65, 0x73, 0x2d, 0x73, 0x79, 0x6e, 0x63,
                0x00, 0x01
            ]
        );
    }

    // AIDEV-NOTE: Golden UUID for "code-style.md". Changing it breaks compat
    // with previously-synced rules. Update only with a major version bump.
    #[test]
    fn prompt_id_for_filename_has_golden_value() {
        let PromptId::User { uuid } = prompt_id_for_filename("code-style.md") else {
            panic!("expected User variant");
        };
        assert_eq!(uuid.0.to_string(), "55301176-2e7a-5a33-8aba-7b7b0019ec40");
    }

    #[test]
    fn prompt_id_for_filename_is_deterministic() {
        assert_eq!(
            prompt_id_for_filename("code-style.md"),
            prompt_id_for_filename("code-style.md"),
        );
    }

    #[test]
    fn title_from_filename_humanizes_slugs() {
        assert_eq!(title_from_filename("code-style.md"), "Code Style");
        assert_eq!(
            title_from_filename("rust_conventions.md"),
            "Rust Conventions"
        );
        assert_eq!(title_from_filename("my-rule.md"), "My Rule");
        assert_eq!(title_from_filename("single.md"), "Single");
    }

    #[test]
    fn title_to_filename_slugifies() {
        assert_eq!(title_to_filename("Code Style"), "code-style.md");
        assert_eq!(title_to_filename("Rust Conventions"), "rust-conventions.md");
        assert_eq!(title_to_filename("My Rule"), "my-rule.md");
    }

    // AIDEV-NOTE: is_managed relies on filename↔title being a stable round trip
    // for slug-form names. If this breaks, --prune and --managed silently stop
    // recognizing rules the tool created.
    #[test]
    fn slug_filenames_round_trip_through_title() {
        for fname in ["code-style.md", "rust-conventions.md", "my-rule.md"] {
            let title = title_from_filename(fname);
            assert_eq!(
                title_to_filename(&title),
                fname,
                "round trip failed for {fname}"
            );
        }
    }

    #[test]
    fn is_managed_true_for_tool_created_rules() {
        let id = prompt_id_for_filename("code-style.md");
        assert!(is_managed(&id, Some("Code Style")));
    }

    #[test]
    fn is_managed_false_when_uuid_does_not_match_title() {
        let id = prompt_id_for_filename("code-style.md");
        assert!(!is_managed(&id, Some("Unrelated Rule")));
    }

    #[test]
    fn is_managed_false_for_builtin_prompts() {
        let id = PromptId::BuiltIn(BuiltInPrompt::CommitMessage);
        assert!(!is_managed(&id, Some("Any Title")));
    }

    #[test]
    fn is_managed_false_when_title_missing() {
        let id = prompt_id_for_filename("code-style.md");
        assert!(!is_managed(&id, None));
    }
}
