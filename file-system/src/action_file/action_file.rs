use ceramic_core::{Base64String, StreamId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionFile {
    pub file_name: String,
    pub file_type: u64,
    pub fs_version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub access_control: Option<String>,

    pub deleted: Option<bool>,
    pub reserved: Option<String>,

    pub action: Base64String,
    // must be a file or union
    pub relation_id: StreamId,
}

impl ActionFile {
    pub fn action(&self) -> anyhow::Result<Action> {
        Ok(serde_json::from_slice(&self.action.to_vec()?)?)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    action_type: ActionType,
    comment: Option<String>,
    is_relation_id_encrypted: Option<bool>,
    is_comment_encrypted: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Like,
    Comment,
    SecretClick,
    Collect,
    Unlock,
    Receive,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_action_file() {
        let action_file = serde_json::json!({
          "action": "eyJhY3Rpb25UeXBlIjoiTElLRSIsImNvbW1lbnQiOiJJIGxpa2UgaXQhIiwiaXNSZWxhdGlvbklkRW5jcnlwdGVkIjpmYWxzZSwiaXNDb21tZW50RW5jcnlwdGVkIjpmYWxzZX0",
          "fileName": "like",
          "fileType": 0,
          "createdAt": "2023-09-22T07:31:03.206Z",
          "fsVersion": "0.11",
          "updatedAt": "2023-09-22T07:31:03.206Z",
          "relationId": "kjzl6kcym7w8yaejed4nbzi4lisljvo1bklovqr4251l93x04064fozndciadha"
        });

        let action_file = serde_json::from_value::<ActionFile>(action_file);
        assert!(action_file.is_ok());
        let action_file = action_file.unwrap();
        let action = action_file.action();
        assert!(action.is_ok());
    }

    #[test]
    fn test_deserialize_action() {
        let content = "eyJhY3Rpb25UeXBlIjoiTElLRSIsImNvbW1lbnQiOiJJIGxpa2UgaXQhIiwiaXNSZWxhdGlvbklkRW5jcnlwdGVkIjpmYWxzZSwiaXNDb21tZW50RW5jcnlwdGVkIjpmYWxzZX0";
        let content = Base64String::from(content.to_string());
        let action = serde_json::from_slice::<Action>(&content.to_vec().unwrap());
        assert!(action.is_ok());
        let action = action.unwrap();
        assert_eq!(
            action,
            Action {
                action_type: ActionType::Like,
                comment: Some("I like it!".to_string()),
                is_relation_id_encrypted: Some(false),
                is_comment_encrypted: Some(false)
            }
        )
    }
}
