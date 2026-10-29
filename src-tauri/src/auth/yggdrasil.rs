use crate::auth::error::AuthError;
use crate::auth::models::AccountProfile;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ELY_BY_AUTH_SERVER: &str = "https://authserver.ely.by";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthenticateRequest<'a> {
    username: &'a str,
    password: String,
    client_token: Uuid,
    request_user: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshRequest<'a> {
    access_token: &'a str,
    client_token: Uuid,
    request_user: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenRequest<'a> {
    access_token: &'a str,
    client_token: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateRequest<'a> {
    access_token: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub access_token: String,
    pub client_token: Uuid,
    pub selected_profile: Option<AccountProfile>,
    #[serde(default)]
    pub available_profiles: Vec<AccountProfile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderError {
    error: Option<String>,
    error_message: Option<String>,
}

#[derive(Clone)]
pub struct YggdrasilClient {
    http: Client,
    base_url: String,
}

impl Default for YggdrasilClient {
    fn default() -> Self {
        Self::new(ELY_BY_AUTH_SERVER)
    }
}

impl YggdrasilClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }

    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
        totp: Option<&str>,
        client_token: Uuid,
    ) -> Result<TokenResponse, AuthError> {
        if username.trim().is_empty() || password.is_empty() {
            return Err(AuthError::InvalidAccount(
                "Username and password are required".into(),
            ));
        }
        if let Some(code) = totp {
            if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(AuthError::InvalidAccount(
                    "The two-factor code must contain six digits".into(),
                ));
            }
        }
        let password = match totp {
            Some(code) => format!("{password}:{code}"),
            None => password.to_owned(),
        };
        let request = AuthenticateRequest {
            username: username.trim(),
            password,
            client_token,
            request_user: true,
        };
        let response = self
            .http
            .post(format!("{}/auth/authenticate", self.base_url))
            .json(&request)
            .send()
            .await?;
        let token = validate_token_response(parse_response(response).await?, client_token)?;
        if token.selected_profile.is_none() && token.available_profiles.is_empty() {
            return Err(AuthError::MalformedResponse);
        }
        Ok(token)
    }

    pub async fn refresh(
        &self,
        access_token: &str,
        client_token: Uuid,
    ) -> Result<TokenResponse, AuthError> {
        let response = self
            .http
            .post(format!("{}/auth/refresh", self.base_url))
            .json(&RefreshRequest {
                access_token,
                client_token,
                request_user: true,
            })
            .send()
            .await?;
        validate_token_response(parse_response(response).await?, client_token)
    }

    pub async fn validate(&self, access_token: &str) -> Result<bool, AuthError> {
        let response = self
            .http
            .post(format!("{}/auth/validate", self.base_url))
            .json(&ValidateRequest { access_token })
            .send()
            .await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(true),
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED => Ok(false),
            _ => Err(provider_error(response).await),
        }
    }

    pub async fn invalidate(
        &self,
        access_token: &str,
        client_token: Uuid,
    ) -> Result<(), AuthError> {
        let response = self
            .http
            .post(format!("{}/auth/invalidate", self.base_url))
            .json(&TokenRequest {
                access_token,
                client_token,
            })
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(provider_error(response).await)
        }
    }
}

async fn parse_response(response: Response) -> Result<TokenResponse, AuthError> {
    if !response.status().is_success() {
        return Err(provider_error(response).await);
    }
    response
        .json::<TokenResponse>()
        .await
        .map_err(|_| AuthError::MalformedResponse)
}

fn validate_token_response(
    token: TokenResponse,
    expected_client_token: Uuid,
) -> Result<TokenResponse, AuthError> {
    if token.client_token != expected_client_token || token.access_token.is_empty() {
        return Err(AuthError::MalformedResponse);
    }
    Ok(token)
}

async fn provider_error(response: Response) -> AuthError {
    let status = response.status();
    let body = response.json::<ProviderError>().await;
    match body {
        Ok(body) => AuthError::Provider {
            kind: body.error.unwrap_or_else(|| status.to_string()),
            message: body
                .error_message
                .unwrap_or_else(|| "The authentication request was rejected".into()),
        },
        Err(_) => AuthError::Provider {
            kind: status.to_string(),
            message: "The authentication request was rejected".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_token_response, AuthenticateRequest, TokenResponse, ValidateRequest};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn parses_yggdrasil_auth_response_fields() {
        let client_token = Uuid::new_v4();
        let data = json!({
            "accessToken": "test-access-token",
            "clientToken": client_token,
            "selectedProfile": { "id": Uuid::new_v4(), "name": "Player" },
            "availableProfiles": []
        });
        let parsed: TokenResponse = serde_json::from_value(data).expect("valid response");
        assert_eq!(parsed.client_token, client_token);
        assert_eq!(parsed.selected_profile.as_ref().unwrap().name, "Player");
        assert_eq!(parsed.access_token, "test-access-token");
    }

    #[test]
    fn malformed_token_response_is_not_accepted() {
        assert!(
            serde_json::from_value::<TokenResponse>(json!({ "accessToken": "missing" })).is_err()
        );
    }

    #[test]
    fn validate_request_matches_ely_by_schema() {
        let request = ValidateRequest {
            access_token: "private-token",
        };
        let value = serde_json::to_value(request).expect("serialize request");
        assert_eq!(value["accessToken"], "private-token");
        assert!(value.get("clientToken").is_none());
    }

    #[test]
    fn totp_is_attached_to_password_only_for_the_auth_request() {
        let request = AuthenticateRequest {
            username: "user@example.org",
            password: "password:123456".into(),
            client_token: Uuid::nil(),
            request_user: true,
        };
        let value = serde_json::to_value(request).expect("serialize request");
        assert_eq!(value["password"], "password:123456");
        assert_eq!(value["clientToken"], Uuid::nil().to_string());
        assert_eq!(value["requestUser"], true);
    }

    #[test]
    fn token_response_must_echo_our_client_token() {
        let response: TokenResponse = serde_json::from_value(json!({
            "accessToken": "token",
            "clientToken": Uuid::new_v4(),
            "selectedProfile": { "id": Uuid::new_v4(), "name": "Player" }
        }))
        .expect("response shape");
        assert!(validate_token_response(response, Uuid::nil()).is_err());
    }
}
