use crate::proxy::auth::{Extensions, Whitelist};
use base64::Engine;
use http::{header, HeaderMap};
use std::net::{IpAddr, SocketAddr};

/// Auth Error
#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    ProxyAuthenticationRequired,
    #[error("Forbidden")]
    Forbidden,
}

/// Enum representing different types of authenticators.
#[derive(Clone)]
pub enum Authenticator {
    /// No authentication with an IP whitelist.
    None(Vec<IpAddr>),
    /// Password authentication with a username, password, and IP whitelist.
    Password {
        username: String,
        password: String,
        whitelist: Vec<IpAddr>,
    },
}

impl Whitelist for Authenticator {
    fn is_empty(&self) -> bool {
        let whitelist = match self {
            Authenticator::None(whitelist) => whitelist,
            Authenticator::Password { whitelist, .. } => whitelist,
        };

        // Check if the whitelist is empty
        whitelist.is_empty()
    }

    fn contains(&self, ip: IpAddr) -> bool {
        let whitelist = match self {
            Authenticator::None(whitelist) => whitelist,
            Authenticator::Password { whitelist, .. } => whitelist,
        };

        // If whitelist is empty, allow all
        whitelist.contains(&ip)
    }
}

impl Authenticator {
    pub fn authenticate(
        &self,
        headers: &HeaderMap,
        socket: SocketAddr,
    ) -> Result<Extensions, AuthError> {
        match self {
            Authenticator::None(..) => {
                // If whitelist is empty, allow all
                let is_equal = self.contains(socket.ip()) || self.is_empty();
                if !is_equal {
                    tracing::warn!("Unauthorized access from {}", socket);
                    return Err(AuthError::Forbidden);
                }
                Ok(Extensions::None)
            }
            Authenticator::Password {
                username, password, ..
            } => {
                // Extract basic auth
                let basic_auth = headers
                    .get(header::PROXY_AUTHORIZATION)
                    .and_then(|hv| hv.to_str().ok())
                    .and_then(|s| s.strip_prefix("Basic "))
                    .ok_or_else(|| AuthError::ProxyAuthenticationRequired)?;

                // Convert to string
                let auth_bytes = base64::engine::general_purpose::STANDARD
                    .decode(basic_auth.as_bytes())
                    .map_err(|_| AuthError::ProxyAuthenticationRequired)?;

                let auth_str = String::from_utf8(auth_bytes)
                    .map_err(|_| AuthError::ProxyAuthenticationRequired)?;

                let (auth_username, auth_password) = auth_str
                    .split_once(':')
                    .ok_or_else(|| AuthError::ProxyAuthenticationRequired)?;

                // Check if the username and password are correct
                let is_equal =
                    ({ auth_username.starts_with(&*username) && auth_password.eq(&*password) })
                        || self.contains(socket.ip());

                // Check credentials
                if is_equal {
                    Ok(Extensions::from((username.as_str(), auth_username)))
                } else {
                    tracing::warn!("Unauthorized access from {}", socket);
                    return Err(AuthError::Forbidden);
                }
            }
        }
    }
}
