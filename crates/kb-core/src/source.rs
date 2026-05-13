use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum Source {
    Human,
    ClaudeCode {
        session_id: String,
        project: String,
        cwd: String,
    },
    Imported {
        from: String,
        original_date: chrono::DateTime<chrono::Utc>,
    },
}
