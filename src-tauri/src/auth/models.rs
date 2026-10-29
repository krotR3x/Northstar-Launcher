use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthProvider {
    Microsoft,
    ElyBy,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountMetadata {
    pub account_id: Uuid,
    pub profile: AccountProfile,
    pub provider: AuthProvider,
    pub active: bool,
    pub skin_path: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedSession {
    pub account_id: Uuid,
    pub profile: AccountProfile,
    pub client_token: Uuid,
    pub access_token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_metadata_never_contains_credentials() {
        let metadata = AccountMetadata {
            account_id: Uuid::nil(),
            profile: AccountProfile {
                id: Uuid::nil(),
                name: "Player".into(),
            },
            provider: AuthProvider::ElyBy,
            active: true,
            skin_path: None,
        };
        let value = serde_json::to_value(metadata).expect("metadata serializes");
        assert!(value.get("accessToken").is_none());
        assert!(value.get("refreshToken").is_none());
        assert_eq!(value["provider"], "ely_by");
    }
}
