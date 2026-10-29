use crate::auth::error::AuthError;
use crate::auth::models::{AccountMetadata, AccountProfile, AuthProvider};
use crate::config::{ProjectDirsExt, LAUNCHER_DIRECTORY};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccount {
    account_id: Uuid,
    profile: AccountProfile,
    provider: AuthProvider,
    client_token: Option<Uuid>,
    active: bool,
    #[serde(default)]
    skin_path: Option<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountFile {
    #[serde(default)]
    accounts: Vec<StoredAccount>,
}

pub struct AccountStore {
    path: PathBuf,
    accounts: Mutex<Vec<StoredAccount>>,
}

impl AccountStore {
    pub fn new(path: PathBuf) -> Result<Self, AuthError> {
        let accounts = if path.exists() {
            let raw = std::fs::read(&path).map_err(|_| AuthError::Storage)?;
            serde_json::from_slice::<AccountFile>(&raw)
                .map_err(|_| AuthError::Storage)?
                .accounts
        } else {
            Vec::new()
        };
        Ok(Self {
            path,
            accounts: Mutex::new(accounts),
        })
    }

    fn default_path() -> PathBuf {
        LAUNCHER_DIRECTORY
            .root_dir()
            .join("northstar-accounts.json")
    }

    pub fn default_store() -> Result<&'static Self, AuthError> {
        static STORE: OnceLock<AccountStore> = OnceLock::new();
        if let Some(store) = STORE.get() {
            return Ok(store);
        }
        let store = Self::new(Self::default_path())?;
        let _ = STORE.set(store);
        STORE.get().ok_or(AuthError::Storage)
    }

    pub async fn list(&self) -> Vec<AccountMetadata> {
        self.accounts
            .lock()
            .await
            .iter()
            .map(|account| AccountMetadata {
                account_id: account.account_id,
                profile: account.profile.clone(),
                provider: account.provider,
                active: account.active,
                skin_path: account
                    .skin_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            })
            .collect()
    }

    pub async fn add_offline(&self, profile: AccountProfile) -> Result<AccountMetadata, AuthError> {
        let mut accounts = self.accounts.lock().await;
        if accounts.iter().any(|account| {
            account.provider == AuthProvider::Offline
                && account.profile.name.eq_ignore_ascii_case(&profile.name)
        }) {
            return Err(AuthError::InvalidAccount(
                "An offline account with this username already exists".into(),
            ));
        }
        let active = accounts.is_empty() || !accounts.iter().any(|account| account.active);
        if active {
            for account in accounts.iter_mut() {
                account.active = false;
            }
        }
        let account = StoredAccount {
            account_id: Uuid::new_v4(),
            profile,
            provider: AuthProvider::Offline,
            client_token: None,
            active,
            skin_path: None,
        };
        accounts.push(account.clone());
        if let Err(error) = save_accounts(&self.path, &accounts) {
            accounts.pop();
            return Err(error);
        }
        Ok(metadata(&account))
    }

    pub async fn add_ely(
        &self,
        profile: AccountProfile,
        client_token: Uuid,
        access_token: &str,
    ) -> Result<AccountMetadata, AuthError> {
        let account_id = Uuid::new_v4();
        secure_store(account_id, access_token).await?;
        let mut accounts = self.accounts.lock().await;
        let active = accounts.is_empty() || !accounts.iter().any(|account| account.active);
        if active {
            for account in accounts.iter_mut() {
                account.active = false;
            }
        }
        let account = StoredAccount {
            account_id,
            profile,
            provider: AuthProvider::ElyBy,
            client_token: Some(client_token),
            active,
            skin_path: None,
        };
        accounts.push(account.clone());
        if let Err(error) = save_accounts(&self.path, &accounts) {
            accounts.pop();
            let _ = secure_clear(account_id).await;
            return Err(error);
        }
        Ok(metadata(&account))
    }

    pub async fn account(&self, id: Uuid) -> Result<(AccountMetadata, Option<Uuid>), AuthError> {
        self.accounts
            .lock()
            .await
            .iter()
            .find(|account| account.account_id == id)
            .map(|account| (metadata(account), account.client_token))
            .ok_or(AuthError::AccountNotFound)
    }

    pub async fn token(&self, id: Uuid) -> Result<String, AuthError> {
        let (account, _) = self.account(id).await?;
        if account.provider != AuthProvider::ElyBy {
            return Err(AuthError::InvalidAccount(
                "This account does not use Ely.by credentials".into(),
            ));
        }
        secure_lookup(id).await
    }

    pub async fn replace_token(&self, id: Uuid, token: &str) -> Result<(), AuthError> {
        let (account, _) = self.account(id).await?;
        if account.provider != AuthProvider::ElyBy {
            return Err(AuthError::InvalidAccount(
                "This account does not use Ely.by credentials".into(),
            ));
        }
        secure_store(id, token).await
    }

    pub async fn set_active(&self, id: Uuid) -> Result<(), AuthError> {
        let mut accounts = self.accounts.lock().await;
        if !accounts.iter().any(|account| account.account_id == id) {
            return Err(AuthError::AccountNotFound);
        }
        for account in accounts.iter_mut() {
            account.active = account.account_id == id;
        }
        save_accounts(&self.path, &accounts)
    }

    pub async fn remove(&self, id: Uuid) -> Result<(), AuthError> {
        let mut accounts = self.accounts.lock().await;
        let index = accounts
            .iter()
            .position(|account| account.account_id == id)
            .ok_or(AuthError::AccountNotFound)?;
        let removed = accounts.remove(index);
        if removed.active {
            if let Some(next) = accounts.first_mut() {
                next.active = true;
            }
        }
        if let Err(error) = save_accounts(&self.path, &accounts) {
            accounts.insert(index, removed);
            return Err(error);
        }
        if removed.provider == AuthProvider::ElyBy {
            secure_clear(id).await?;
        }
        Ok(())
    }

    pub async fn set_skin_path(&self, id: Uuid, path: Option<PathBuf>) -> Result<(), AuthError> {
        if let Some(path) = path.as_ref() {
            validate_skin_file(path)?;
        }
        let mut accounts = self.accounts.lock().await;
        let index = accounts
            .iter_mut()
            .position(|account| account.account_id == id)
            .ok_or(AuthError::AccountNotFound)?;
        let previous = accounts[index].skin_path.clone();
        accounts[index].skin_path = path;
        if let Err(error) = save_accounts(&self.path, &accounts) {
            accounts[index].skin_path = previous;
            return Err(error);
        }
        Ok(())
    }
}

fn metadata(account: &StoredAccount) -> AccountMetadata {
    AccountMetadata {
        account_id: account.account_id,
        profile: account.profile.clone(),
        provider: account.provider,
        active: account.active,
        skin_path: account
            .skin_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    }
}

fn save_accounts(path: &Path, accounts: &[StoredAccount]) -> Result<(), AuthError> {
    let parent = path.parent().ok_or(AuthError::Storage)?;
    std::fs::create_dir_all(parent).map_err(|_| AuthError::Storage)?;
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let data = serde_json::to_vec_pretty(&AccountFile {
        accounts: accounts.to_vec(),
    })
    .map_err(|_| AuthError::Storage)?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| AuthError::Storage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| AuthError::Storage)?;
    }
    file.write_all(&data).map_err(|_| AuthError::Storage)?;
    file.sync_all().map_err(|_| AuthError::Storage)?;
    std::fs::rename(&temporary, path).map_err(|_| AuthError::Storage)
}

pub fn validate_skin_file(path: &Path) -> Result<PathBuf, AuthError> {
    let path = std::fs::canonicalize(path)
        .map_err(|_| AuthError::InvalidAccount("Skin file cannot be opened".into()))?;
    let metadata = std::fs::metadata(&path)
        .map_err(|_| AuthError::InvalidAccount("Skin file cannot be opened".into()))?;
    if !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
        return Err(AuthError::InvalidAccount(
            "Skin must be a regular image file no larger than 4 MiB".into(),
        ));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if extension.as_deref() != Some("png") {
        return Err(AuthError::InvalidAccount(
            "Minecraft skin image must use PNG format".into(),
        ));
    }
    let bytes = std::fs::read(&path)
        .map_err(|_| AuthError::InvalidAccount("Skin file cannot be read".into()))?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(AuthError::InvalidAccount(
            "Skin file contents are not a PNG image".into(),
        ));
    }
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|_| AuthError::InvalidAccount("Skin PNG could not be decoded".into()))?;
    if image.width() != 64 || !matches!(image.height(), 32 | 64) {
        return Err(AuthError::InvalidAccount(
            "Minecraft skin dimensions must be 64×64 or legacy 64×32 pixels".into(),
        ));
    }
    Ok(path)
}

async fn secure_store(account_id: Uuid, secret: &str) -> Result<(), AuthError> {
    let id = account_id.to_string();
    let secret = secret.to_owned();
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let mut child = secret_tool()
                .arg("store")
                .arg("--label=Northstar Launcher Ely.by session")
                .arg("service")
                .arg("northstar-launcher-ely-by")
                .arg("account")
                .arg(id)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| AuthError::SecureStorageUnavailable)?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or(AuthError::SecureStorageUnavailable)?;
            stdin
                .write_all(secret.as_bytes())
                .map_err(|_| AuthError::SecureStorageUnavailable)?;
            drop(stdin);
            let status = child
                .wait()
                .map_err(|_| AuthError::SecureStorageUnavailable)?;
            if status.success() {
                Ok(())
            } else {
                Err(AuthError::SecureStorageUnavailable)
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (id, secret);
            Err(AuthError::SecureStorageUnsupportedPlatform)
        }
    })
    .await
    .map_err(|_| AuthError::SecureStorageUnavailable)?
}

async fn secure_lookup(account_id: Uuid) -> Result<String, AuthError> {
    let id = account_id.to_string();
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let output = secret_tool()
                .arg("lookup")
                .arg("service")
                .arg("northstar-launcher-ely-by")
                .arg("account")
                .arg(id)
                .output()
                .map_err(|_| AuthError::SecureStorageUnavailable)?;
            if !output.status.success() {
                return Err(AuthError::SecureStorageUnavailable);
            }
            String::from_utf8(output.stdout)
                .map(|value| value.trim_end_matches(['\r', '\n']).to_owned())
                .map_err(|_| AuthError::SecureStorageUnavailable)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = id;
            Err(AuthError::SecureStorageUnsupportedPlatform)
        }
    })
    .await
    .map_err(|_| AuthError::SecureStorageUnavailable)?
}

async fn secure_clear(account_id: Uuid) -> Result<(), AuthError> {
    let id = account_id.to_string();
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let status = secret_tool()
                .arg("clear")
                .arg("service")
                .arg("northstar-launcher-ely-by")
                .arg("account")
                .arg(id)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|_| AuthError::SecureStorageUnavailable)?;
            if status.success() {
                Ok(())
            } else {
                Err(AuthError::SecureStorageUnavailable)
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = id;
            Err(AuthError::SecureStorageUnsupportedPlatform)
        }
    })
    .await
    .map_err(|_| AuthError::SecureStorageUnavailable)?
}

#[cfg(target_os = "linux")]
fn secret_tool() -> Command {
    // Do not search PATH: this command receives session credentials over stdin.
    Command::new("/usr/bin/secret-tool")
}

#[cfg(test)]
mod tests {
    use super::validate_skin_file;
    use std::fs;

    #[test]
    fn skin_validation_checks_extension_and_image_signature() {
        let directory =
            std::env::temp_dir().join(format!("northstar-skin-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).expect("create test dir");
        let png = directory.join("skin.png");
        image::RgbaImage::new(64, 64)
            .save(&png)
            .expect("create PNG fixture");
        assert!(validate_skin_file(&png).is_ok());
        let invalid = directory.join("bad.png");
        fs::write(&invalid, b"not a PNG").expect("create invalid fixture");
        assert!(validate_skin_file(&invalid).is_err());
        let _ = fs::remove_dir_all(directory);
    }
}
