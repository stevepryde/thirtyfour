use serde::{Deserialize, Serialize};

/// Enum representing the `SameSite` attribute of a cookie.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SameSite {
    /// Strict `SameSite` attribute.
    Strict,
    /// Lax `SameSite` attribute.
    Lax,
    /// No `SameSite` attribute.
    None,
}

/// Cookie struct used to create new cookies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    /// The name of the cookie.
    pub name: String,
    /// The value of the cookie.
    pub value: String,
    /// The path of the cookie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The domain of the cookie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Whether the cookie is secure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    /// The expiry date of the cookie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<i64>,
    /// The sameSite attribute of the cookie.
    #[serde(skip_serializing_if = "Option::is_none", rename = "sameSite")]
    pub same_site: Option<SameSite>,
}

impl Cookie {
    /// Create a new Cookie struct, specifying the name and the JSON data.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Cookie {
            name: name.into(),
            value: value.into(),
            path: None,
            domain: None,
            secure: None,
            expiry: None,
            same_site: None,
        }
    }

    /// Set the path of the cookie.
    pub fn set_path(&mut self, path: impl Into<String>) {
        self.path = Some(path.into());
    }

    /// Set the domain of the cookie.
    pub fn set_domain(&mut self, domain: impl Into<String>) {
        self.domain = Some(domain.into());
    }

    /// Set whether the cookie is secure.
    pub fn set_secure(&mut self, secure: bool) {
        self.secure = Some(secure);
    }

    /// Set the expiry date of the cookie.
    pub fn set_expiry(&mut self, expiry: i64) {
        self.expiry = Some(expiry);
    }

    /// Set the sameSite attribute of the cookie.
    pub fn set_same_site(&mut self, same_site: SameSite) {
        self.same_site = Some(same_site);
    }
}
