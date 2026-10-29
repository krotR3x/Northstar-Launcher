use crate::auth::error::AuthError;
use crate::auth::models::{AccountMetadata, AuthProvider};
use crate::auth::offline::offline_profile;
use crate::auth::storage::AccountStore;
use crate::auth::yggdrasil::YggdrasilClient;
use crate::error::{AppError, CommandError};
use serde::Serialize;
use uuid::Uuid;

type Result<T> = std::result::Result<T, CommandError>;

fn map_error(error: AuthError) -> CommandError {
    CommandError::from(AppError::Other(error.to_string()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElyLoginResult {
    pub account: AccountMetadata,
    pub available_profiles: Vec<crate::auth::models::AccountProfile>,
}

#[tauri::command]
pub async fn list_northstar_accounts() -> Result<Vec<AccountMetadata>> {
    Ok(AccountStore::default_store()
        .map_err(map_error)?
        .list()
        .await)
}

#[tauri::command]
pub async fn create_offline_account(username: String) -> Result<AccountMetadata> {
    let profile = offline_profile(&username).map_err(map_error)?;
    AccountStore::default_store()
        .map_err(map_error)?
        .add_offline(profile)
        .await
        .map_err(map_error)
}

/// Authenticates with Ely.by's documented Yggdrasil authenticate endpoint. The password and
/// optional TOTP are sent only in this request; neither is written to the account file.
#[tauri::command]
pub async fn authenticate_ely_by(
    username: String,
    password: String,
    totp: Option<String>,
) -> Result<ElyLoginResult> {
    let client_token = Uuid::new_v4();
    let response = YggdrasilClient::default()
        .authenticate(&username, &password, totp.as_deref(), client_token)
        .await
        .map_err(map_error)?;
    drop(password);
    drop(totp);
    let profile = match response.selected_profile {
        Some(profile) => profile,
        None if response.available_profiles.len() == 1 => response.available_profiles[0].clone(),
        None => {
            return Err(map_error(AuthError::InvalidAccount(
                "Ely.by did not select a single Minecraft profile".into(),
            )))
        }
    };
    let available_profiles = response.available_profiles.clone();
    let account = AccountStore::default_store()
        .map_err(map_error)?
        .add_ely(profile, response.client_token, &response.access_token)
        .await
        .map_err(map_error)?;
    Ok(ElyLoginResult {
        account,
        available_profiles,
    })
}

#[tauri::command]
pub async fn validate_ely_account(account_id: Uuid) -> Result<bool> {
    let store = AccountStore::default_store().map_err(map_error)?;
    let (account, _) = store.account(account_id).await.map_err(map_error)?;
    if account.provider != AuthProvider::ElyBy {
        return Err(map_error(AuthError::InvalidAccount(
            "Token validation is only available for Ely.by accounts".into(),
        )));
    }
    let access_token = store.token(account_id).await.map_err(map_error)?;
    let valid = YggdrasilClient::default()
        .validate(&access_token)
        .await
        .map_err(map_error)?;
    drop(access_token);
    Ok(valid)
}

#[tauri::command]
pub async fn refresh_ely_account(account_id: Uuid) -> Result<AccountMetadata> {
    let store = AccountStore::default_store().map_err(map_error)?;
    let (_, client_token) = store.account(account_id).await.map_err(map_error)?;
    let client_token = client_token.ok_or_else(|| {
        map_error(AuthError::InvalidAccount(
            "Ely.by client token is missing".into(),
        ))
    })?;
    let old_token = store.token(account_id).await.map_err(map_error)?;
    let response = YggdrasilClient::default()
        .refresh(&old_token, client_token)
        .await
        .map_err(map_error)?;
    drop(old_token);
    store
        .replace_token(account_id, &response.access_token)
        .await
        .map_err(map_error)?;
    store
        .account(account_id)
        .await
        .map(|(account, _)| account)
        .map_err(map_error)
}

#[tauri::command]
pub async fn set_northstar_active_account(account_id: Uuid) -> Result<()> {
    AccountStore::default_store()
        .map_err(map_error)?
        .set_active(account_id)
        .await
        .map_err(map_error)
}

#[tauri::command]
pub async fn remove_northstar_account(account_id: Uuid) -> Result<()> {
    let store = AccountStore::default_store().map_err(map_error)?;
    if let Ok((account, client_token)) = store.account(account_id).await {
        if account.provider == AuthProvider::ElyBy {
            match (store.token(account_id).await, client_token) {
                (Ok(token), Some(client_token)) => {
                    if let Err(error) = YggdrasilClient::default()
                        .invalidate(&token, client_token)
                        .await
                    {
                        log::warn!("Ely.by token invalidation failed during local logout: {error}");
                    }
                }
                (Err(error), _) => log::warn!("Could not read Ely.by token during logout: {error}"),
                (_, None) => log::warn!("Ely.by client token missing during logout"),
            }
        }
    }
    store.remove(account_id).await.map_err(map_error)
}

#[tauri::command]
pub async fn set_northstar_skin(account_id: Uuid, path: Option<String>) -> Result<()> {
    let path = path.map(std::path::PathBuf::from);
    AccountStore::default_store()
        .map_err(map_error)?
        .set_skin_path(account_id, path)
        .await
        .map_err(map_error)
}
