//! `App`: all UI state, and the operations on it.
use std::collections::HashMap;

use uuid::Uuid;

use crate::db::Conn;
use crate::model::{Connection, SslMode};
use crate::{logger, secret, storage};

/// Live state of a saved connection; a connection with no entry is disconnected.
pub enum ConnState {
    Connecting,
    Connected(Conn),
    Failed(String),
}

pub enum Popup {
    Form(ConnForm),
    ConfirmDelete(Uuid),
}

pub struct App {
    pub connections: Vec<Connection>,
    pub states: HashMap<Uuid, ConnState>,
    pub selected: usize,
    pub popup: Option<Popup>,
    pub status_message: String,
}

impl App {
    pub fn new(connections: Vec<Connection>) -> App {
        App {
            connections,
            states: HashMap::new(),
            selected: 0,
            popup: None,
            status_message: String::new(),
        }
    }

    pub fn selected_connection(&self) -> Option<&Connection> {
        self.connections.get(self.selected)
    }

    pub fn move_selection(&mut self, delta: isize) {
        let last = self.connections.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    pub fn open_add_form(&mut self) {
        self.popup = Some(Popup::Form(ConnForm::new()));
    }

    pub fn open_edit_form(&mut self) {
        if let Some(c) = self.selected_connection() {
            self.popup = Some(Popup::Form(ConnForm::edit(c)));
        }
    }

    /// Validate the open form and save it: the password to the keychain (if it was
    /// typed), the connection to connections.json. On failure the form stays open.
    pub fn save_form(&mut self) {
        let Some(Popup::Form(form)) = &mut self.popup else {
            return;
        };
        let conn = match form.to_connection() {
            Ok(c) => c,
            Err(e) => {
                form.error = Some(e);
                return;
            }
        };
        if form.password_edited
            && let Err(e) = secret::set_password(conn.id, &form.fields[PASSWORD])
        {
            form.error = Some(format!("Couldn't save the password to the keychain: {e}"));
            return;
        }
        let name = conn.name.clone();
        match self.connections.iter().position(|c| c.id == conn.id) {
            Some(i) => self.connections[i] = conn,
            None => {
                self.connections.push(conn);
                self.selected = self.connections.len() - 1;
            }
        }
        self.popup = None;
        self.status_message = match self.persist() {
            Ok(()) => format!("Saved '{name}'"),
            Err(e) => e,
        };
    }

    pub fn delete_connection(&mut self, id: Uuid) {
        let Some(i) = self.connections.iter().position(|c| c.id == id) else {
            return;
        };
        let removed = self.connections.remove(i);
        self.states.remove(&id); // drops a live connection, closing it
        self.move_selection(0);
        if let Err(e) = secret::delete_password(id) {
            logger::log("keychain delete", &e.to_string());
        }
        self.status_message = match self.persist() {
            Ok(()) => format!("Deleted '{}'", removed.name),
            Err(e) => e,
        };
    }

    fn persist(&self) -> Result<(), String> {
        storage::save_connections(&self.connections).map_err(|e| {
            logger::log("save connections", &format!("{e:#}"));
            format!("Couldn't save connections: {e:#}")
        })
    }

    /// Mark the selected connection as connecting and return it, unless it's already
    /// connecting or connected. The caller spawns the actual connect.
    pub fn begin_connect(&mut self) -> Option<Connection> {
        let conn = self.selected_connection()?.clone();
        if matches!(
            self.states.get(&conn.id),
            Some(ConnState::Connecting | ConnState::Connected(_))
        ) {
            return None;
        }
        self.states.insert(conn.id, ConnState::Connecting);
        self.status_message = format!("Connecting to '{}'...", conn.name);
        Some(conn)
    }

    /// A connect attempt finished. Ignored if the connection was deleted or
    /// disconnected meanwhile.
    pub fn finish_connect(&mut self, id: Uuid, result: Result<Conn, String>) {
        let Some(state) = self.states.get_mut(&id) else {
            return;
        };
        if !matches!(state, ConnState::Connecting) {
            return;
        }
        let name = self
            .connections
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        *state = match result {
            Ok(conn) => {
                self.status_message = format!("Connected to '{name}' ({})", conn.server_version());
                ConnState::Connected(conn)
            }
            Err(e) => {
                logger::log(&format!("connect '{name}'"), &e);
                self.status_message = format!(
                    "Couldn't connect to '{name}' (logged to {})",
                    logger::path_hint()
                );
                ConnState::Failed(e)
            }
        };
    }

    pub fn disconnect_selected(&mut self) {
        let Some(c) = self.selected_connection() else {
            return;
        };
        let (id, name) = (c.id, c.name.clone());
        if self.states.remove(&id).is_some() {
            self.status_message = format!("Disconnected '{name}'");
        }
    }
}

pub const NAME: usize = 0;
pub const HOST: usize = 1;
pub const PORT: usize = 2;
pub const USER: usize = 3;
pub const PASSWORD: usize = 4;
pub const DATABASE: usize = 5;
/// The TLS row comes after the text fields.
pub const TLS: usize = 6;
pub const FORM_ROWS: usize = 7;
pub const FORM_LABELS: [&str; FORM_ROWS] = [
    "Name", "Host", "Port", "User", "Password", "Database", "TLS",
];

/// The add/edit connection popup.
pub struct ConnForm {
    /// Set when editing an existing connection.
    pub id: Option<Uuid>,
    /// Text fields, indexed by NAME..=DATABASE.
    pub fields: [String; 6],
    pub ssl: SslMode,
    pub focus: usize,
    /// When editing, an untouched password field keeps the saved password.
    pub password_edited: bool,
    pub error: Option<String>,
}

impl ConnForm {
    pub fn new() -> ConnForm {
        ConnForm {
            id: None,
            fields: [
                String::new(),
                "localhost".into(),
                "5432".into(),
                String::new(),
                String::new(),
                String::new(),
            ],
            ssl: SslMode::Disable,
            focus: NAME,
            password_edited: false,
            error: None,
        }
    }

    pub fn edit(c: &Connection) -> ConnForm {
        ConnForm {
            id: Some(c.id),
            fields: [
                c.name.clone(),
                c.host.clone(),
                c.port.to_string(),
                c.user.clone(),
                String::new(),
                c.database.clone(),
            ],
            ssl: c.ssl,
            focus: NAME,
            password_edited: false,
            error: None,
        }
    }

    pub fn move_focus(&mut self, forward: bool) {
        self.focus = if forward {
            (self.focus + 1) % FORM_ROWS
        } else {
            (self.focus + FORM_ROWS - 1) % FORM_ROWS
        };
    }

    pub fn input(&mut self, text: &str) {
        if self.focus == TLS {
            if text == " " {
                self.ssl = self.ssl.toggled();
            }
            return;
        }
        // Single-line fields: a pasted newline would only confuse the DSN.
        self.fields[self.focus].extend(text.chars().filter(|c| !c.is_control()));
        self.password_edited |= self.focus == PASSWORD;
    }

    pub fn backspace(&mut self) {
        if self.focus != TLS {
            self.fields[self.focus].pop();
            self.password_edited |= self.focus == PASSWORD;
        }
    }

    pub fn to_connection(&self) -> Result<Connection, String> {
        let field = |i: usize| self.fields[i].trim().to_string();
        let required = |i: usize| {
            let v = field(i);
            if v.is_empty() {
                Err(format!("{} is required", FORM_LABELS[i]))
            } else {
                Ok(v)
            }
        };
        let name = required(NAME)?;
        let host = required(HOST)?;
        let port = field(PORT)
            .parse::<u16>()
            .map_err(|_| "Port must be a number from 1 to 65535".to_string())?;
        let user = required(USER)?;
        // libpq's default: the database named after the user.
        let database = Some(field(DATABASE))
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| user.clone());
        Ok(Connection {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            name,
            host,
            port,
            user,
            database,
            ssl: self.ssl,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_validates_and_defaults_database_to_user() {
        let mut form = ConnForm::new();
        assert_eq!(form.to_connection().unwrap_err(), "Name is required");
        form.fields[NAME] = "local".into();
        form.fields[USER] = "app".into();
        form.fields[PORT] = "x".into();
        assert!(form.to_connection().unwrap_err().starts_with("Port"));
        form.fields[PORT] = "5433".into();
        let c = form.to_connection().unwrap();
        assert_eq!(c.summary(), "app@localhost:5433/app");
    }

    #[test]
    fn editing_keeps_id_and_leaves_password_alone_until_typed() {
        let _storage = crate::test_support::isolated_storage();
        let mut app = App::new(Vec::new());
        app.open_add_form();
        let Some(Popup::Form(form)) = &mut app.popup else {
            panic!()
        };
        form.fields[NAME] = "local".into();
        form.fields[USER] = "app".into();
        app.save_form();
        assert!(app.popup.is_none());
        let id = app.connections[0].id;
        assert_eq!(storage::load_connections().unwrap().unwrap().len(), 1);

        app.open_edit_form();
        let Some(Popup::Form(form)) = &mut app.popup else {
            panic!()
        };
        assert_eq!(form.id, Some(id));
        assert!(!form.password_edited);
        form.focus = PASSWORD;
        form.input("secret");
        form.backspace();
        assert!(form.password_edited);
        assert_eq!(form.fields[PASSWORD], "secre");
        app.save_form();
        assert_eq!(app.connections.len(), 1);
        assert_eq!(app.connections[0].id, id);
    }

    #[test]
    fn late_connect_result_after_disconnect_is_ignored() {
        let _storage = crate::test_support::isolated_storage();
        let mut form = ConnForm::new();
        form.fields[NAME] = "n".into();
        form.fields[USER] = "u".into();
        let mut app = App::new(vec![form.to_connection().unwrap()]);
        let id = app.begin_connect().unwrap().id;
        assert!(app.begin_connect().is_none(), "already connecting");
        app.disconnect_selected();
        app.finish_connect(id, Err("refused".into()));
        assert!(!app.states.contains_key(&id));
    }
}
