use crate::auth::error::AuthError;
use crate::auth::models::AccountProfile;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Offline UUID algorithm: UUIDv8 with the first 128 bits of SHA-256 over a
/// fixed namespace UUID followed by the ASCII-lowercased username. The same
/// case-insensitive username always maps to the same identity in Northstar.
/// This launcher-local identifier is not a Mojang-issued account UUID.
pub fn offline_profile(username: &str) -> Result<AccountProfile, AuthError> {
    let username = username.trim();
    if !(3..=16).contains(&username.len())
        || !username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(AuthError::InvalidAccount(
            "Offline usernames must be 3–16 ASCII letters, numbers, or underscores".into(),
        ));
    }

    // Fixed UUID namespace for the Northstar offline identity algorithm.
    const NAMESPACE: Uuid = Uuid::from_u128(0xd791_5d2b_2c55_5a32_a928_117f_0f49_82b1);
    let normalized = username.to_ascii_lowercase();
    let mut hash = Sha256::new();
    hash.update(NAMESPACE.as_bytes());
    hash.update([0]);
    hash.update(normalized.as_bytes());
    let digest = hash.finalize();
    let mut uuid_bytes = [0u8; 16];
    uuid_bytes.copy_from_slice(&digest[..16]);
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x80;
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
    let id = Uuid::from_bytes(uuid_bytes);
    Ok(AccountProfile {
        id,
        name: username.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::offline_profile;

    #[test]
    fn offline_uuid_is_stable_and_case_insensitive() {
        let first = offline_profile("NorthStar_1").expect("valid username");
        let second = offline_profile("northstar_1").expect("valid username");
        assert_eq!(first.id, second.id);
        assert_eq!(first.id.get_version_num(), 8);
    }

    #[test]
    fn invalid_offline_names_are_rejected() {
        for name in ["ab", "bad name", "../escape", "éclair"] {
            assert!(offline_profile(name).is_err(), "accepted {name:?}");
        }
    }
}
