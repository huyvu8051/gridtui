//! Serializable data: saved connections. Passwords live in the OS keychain
//! (see `secret.rs`), never in these structs.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// TLS for a connection. Names follow libpq's `sslmode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SslMode {
    #[default]
    Disable,
    /// TLS, with the server certificate checked against the OS trust store.
    // shortcut: no libpq-style `require` (encrypt without verifying); add it when a
    // self-signed server needs it.
    VerifyFull,
}

impl SslMode {
    pub fn label(self) -> &'static str {
        match self {
            SslMode::Disable => "disable",
            SslMode::VerifyFull => "verify-full",
        }
    }

    pub fn toggled(self) -> SslMode {
        match self {
            SslMode::Disable => SslMode::VerifyFull,
            SslMode::VerifyFull => SslMode::Disable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    #[serde(default)]
    pub ssl: SslMode,
}

impl Connection {
    /// `user@host:port/database`
    pub fn summary(&self) -> String {
        format!(
            "{}@{}:{}/{}",
            self.user, self.host, self.port, self.database
        )
    }
}
