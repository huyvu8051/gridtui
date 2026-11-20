//! Database access. Nothing outside `db/` touches a driver crate: everything the rest
//! of the app needs goes through `Conn`. Adding a database means a new module next to
//! `pg.rs` and a new `Conn` variant; the compiler then points at every `match` to extend.
pub mod pg;

use crate::model::Connection;

pub enum Conn {
    Pg(pg::PgConn),
}

impl Conn {
    /// Connect and read the server version. Errors come back ready to show.
    pub async fn connect(connection: &Connection, password: &str) -> Result<Conn, String> {
        pg::PgConn::connect(connection, password)
            .await
            .map(Conn::Pg)
    }

    /// e.g. "PostgreSQL 16.4"
    pub fn server_version(&self) -> &str {
        match self {
            Conn::Pg(c) => &c.version,
        }
    }
}
