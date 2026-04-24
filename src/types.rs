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
