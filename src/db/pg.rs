//! PostgreSQL through tokio-postgres, with TLS by rustls and the OS trust store.
use std::time::Duration;

use tokio_postgres::{Client, NoTls};

use crate::model::{Connection, SslMode};

pub struct PgConn {
    #[allow(dead_code)] // queries arrive in P1
    pub client: Client,
    pub version: String,
}

impl PgConn {
    pub async fn connect(c: &Connection, password: &str) -> Result<PgConn, String> {
        let mut config = tokio_postgres::Config::new();
        config
            .host(&c.host)
            .port(c.port)
            .user(&c.user)
            .dbname(&c.database)
            .application_name("gridtui")
            .connect_timeout(Duration::from_secs(10));
        if !password.is_empty() {
            config.password(password);
        }
        let client = match c.ssl {
            SslMode::Disable => {
                let (client, conn) = config.connect(NoTls).await.map_err(|e| describe(&e))?;
                tokio::spawn(drive(conn));
                client
            }
            SslMode::VerifyFull => {
                config.ssl_mode(tokio_postgres::config::SslMode::Require);
                let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config()?);
                let (client, conn) = config.connect(tls).await.map_err(|e| describe(&e))?;
                tokio::spawn(drive(conn));
                client
            }
        };
        let version: String = client
            .query_one("SHOW server_version", &[])
            .await
            .map_err(|e| describe(&e))?
            .get(0);
        Ok(PgConn {
            client,
            version: format!("PostgreSQL {version}"),
        })
    }
}

/// Run the connection's I/O until it closes; tokio-postgres needs this polled.
async fn drive<F>(conn: F)
where
    F: std::future::Future<Output = Result<(), tokio_postgres::Error>>,
{
    if let Err(e) = conn.await {
        crate::logger::log("postgres connection closed", &describe(&e));
    }
}

fn tls_config() -> Result<rustls::ClientConfig, String> {
    use rustls_platform_verifier::ConfigVerifierExt;
    rustls::ClientConfig::with_platform_verifier().map_err(|e| format!("TLS setup failed: {e}"))
}

/// tokio-postgres's Display is just "db error" for errors the server sent; the real
/// message is on `as_db_error()`. For others (refused, timeout, TLS) add the cause.
fn describe(e: &tokio_postgres::Error) -> String {
    if let Some(db) = e.as_db_error() {
        return format!("{}: {}", db.severity(), db.message());
    }
    let text = e.to_string();
    match std::error::Error::source(e).map(|s| s.to_string()) {
        Some(cause) if !text.contains(&cause) => format!("{text}: {cause}"),
        _ => text,
    }
}
