//! Reading and writing the files in the data directory.
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::model::{Connection, SslMode};

const CONNECTIONS_FILE: &str = "connections.json";

fn connections_path() -> Result<PathBuf> {
    Ok(tuikit::storage::data_dir(crate::APP)?.join(CONNECTIONS_FILE))
}

/// The saved connections; None if the file doesn't exist yet. A corrupt file is an
/// error rather than an empty list, so the next save can't overwrite it (and orphan
/// the passwords in the keychain).
pub fn load_connections() -> Result<Option<Vec<Connection>>> {
    let path = connections_path()?;
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
    };
    serde_json::from_str(&contents)
        .map(Some)
        .with_context(|| format!("{} is corrupt; fix or move it away", path.display()))
}

pub fn save_connections(connections: &[Connection]) -> Result<()> {
    let json = serde_json::to_string_pretty(connections).context("failed to serialize")?;
    tuikit::storage::write_atomic(&connections_path()?, &json)
}

/// Postgres connections saved by datagrust (gridtui's predecessor), with their
/// plaintext passwords. Empty when there is nothing to import.
pub fn read_datagrust_connections() -> Vec<(Connection, String)> {
    let Some(path) = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".config/datagrust/connections.toml"))
    else {
        return Vec::new();
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    parse_datagrust(&contents).unwrap_or_else(|e| {
        crate::logger::log("datagrust import", &format!("{}: {e}", path.display()));
        Vec::new()
    })
}

fn parse_datagrust(toml_text: &str) -> Result<Vec<(Connection, String)>> {
    #[derive(Deserialize)]
    struct File {
        #[serde(default)]
        connections: Vec<Old>,
    }
    #[derive(Deserialize)]
    struct Old {
        name: String,
        #[serde(default)]
        driver: Option<String>,
        host: String,
        port: String,
        user: String,
        #[serde(default)]
        password: String,
        dbname: String,
    }
    let file: File = toml::from_str(toml_text)?;
    Ok(file
        .connections
        .into_iter()
        .filter(|o| o.driver.as_deref().is_none_or(|d| d == "Postgres"))
        .map(|o| {
            let conn = Connection {
                id: Uuid::new_v4(),
                name: o.name,
                host: o.host,
                port: o.port.trim().parse().unwrap_or(5432),
                user: o.user,
                database: o.dbname,
                ssl: SslMode::Disable,
            };
            (conn, o.password)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagrust_import_keeps_postgres_only() {
        let toml_text = r#"
            [[connections]]
            id = 1
            name = "local"
            driver = "Postgres"
            host = "localhost"
            port = "5433"
            user = "app"
            password = "pw"
            dbname = "appdb"

            [[connections]]
            id = 2
            name = "shop"
            driver = "MySql"
            host = "h"
            port = "3306"
            user = "u"
            password = ""
            dbname = "d"

            [[connections]]
            name = "before drivers existed"
            host = "h2"
            port = "5432"
            user = "u2"
            password = ""
            dbname = "d2"
        "#;
        let imported = parse_datagrust(toml_text).unwrap();
        let names: Vec<_> = imported.iter().map(|(c, _)| c.name.as_str()).collect();
        assert_eq!(names, ["local", "before drivers existed"]);
        let (local, password) = &imported[0];
        assert_eq!(local.summary(), "app@localhost:5433/appdb");
        assert_eq!(password, "pw");
    }

    #[test]
    fn connections_round_trip_and_corrupt_file_is_an_error() {
        let _storage = crate::test_support::isolated_storage();
        assert!(load_connections().unwrap().is_none());
        let conn = Connection {
            id: Uuid::new_v4(),
            name: "x".into(),
            host: "h".into(),
            port: 5432,
            user: "u".into(),
            database: "d".into(),
            ssl: SslMode::VerifyFull,
        };
        save_connections(std::slice::from_ref(&conn)).unwrap();
        assert_eq!(load_connections().unwrap(), Some(vec![conn]));
        std::fs::write(connections_path().unwrap(), "[{").unwrap();
        assert!(load_connections().is_err());
    }
}
