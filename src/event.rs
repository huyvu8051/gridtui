//! Key routing: the open popup first, then the connection list.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Popup};
use crate::model::Connection;

pub enum Action {
    None,
    Quit,
    /// Spawn a connect for this connection (already marked as connecting).
    Connect(Connection),
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    match &mut app.popup {
        Some(Popup::Form(form)) => {
            match key.code {
                KeyCode::Esc => app.popup = None,
                KeyCode::Enter => app.save_form(),
                KeyCode::Tab | KeyCode::Down => form.move_focus(true),
                KeyCode::BackTab | KeyCode::Up => form.move_focus(false),
                KeyCode::Backspace => form.backspace(),
                KeyCode::Left | KeyCode::Right => form.input(" "), // toggles TLS, else ignored
                KeyCode::Char(c) if !ctrl => form.input(c.encode_utf8(&mut [0; 4])),
                _ => {}
            }
            return Action::None;
        }
        Some(Popup::ConfirmDelete(id)) => {
            let id = *id;
            app.popup = None;
            if key.code == KeyCode::Char('y') {
                app.delete_connection(id);
            }
            return Action::None;
        }
        None => {}
    }
    match key.code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => app.move_selection(1),
        KeyCode::Char('k') | KeyCode::Up => app.move_selection(-1),
        KeyCode::Char('a') => app.open_add_form(),
        KeyCode::Char('e') => app.open_edit_form(),
        KeyCode::Char('D') => {
            if let Some(c) = app.selected_connection() {
                app.popup = Some(Popup::ConfirmDelete(c.id));
            }
        }
        KeyCode::Enter => {
            if let Some(conn) = app.begin_connect() {
                return Action::Connect(conn);
            }
        }
        KeyCode::Char('x') => app.disconnect_selected(),
        _ => {}
    }
    Action::None
}

pub fn handle_paste(app: &mut App, text: String) {
    if let Some(Popup::Form(form)) = &mut app.popup {
        form.input(&text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{NAME, TLS, USER};
    use crate::model::SslMode;

    fn press(app: &mut App, code: KeyCode) -> Action {
        handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn add_connect_and_delete_with_keys() {
        let _storage = crate::test_support::isolated_storage();
        let mut app = App::new(Vec::new());
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "local"); // Name
        while matches!(&app.popup, Some(Popup::Form(f)) if f.focus != USER) {
            press(&mut app, KeyCode::Tab);
        }
        type_text(&mut app, "app");
        press(&mut app, KeyCode::BackTab);
        while matches!(&app.popup, Some(Popup::Form(f)) if f.focus != TLS) {
            press(&mut app, KeyCode::Tab);
        }
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Enter);
        assert!(app.popup.is_none(), "{:?}", app.status_message);
        assert_eq!(app.connections[0].name, "local");
        assert_eq!(app.connections[0].ssl, SslMode::VerifyFull);

        assert!(matches!(
            press(&mut app, KeyCode::Enter),
            Action::Connect(_)
        ));
        assert!(matches!(press(&mut app, KeyCode::Enter), Action::None));

        press(&mut app, KeyCode::Char('D'));
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.connections.len(), 1);
        press(&mut app, KeyCode::Char('D'));
        press(&mut app, KeyCode::Char('y'));
        assert!(app.connections.is_empty());
        assert!(app.states.is_empty());
    }

    #[test]
    fn typing_q_in_the_form_types_instead_of_quitting() {
        let _storage = crate::test_support::isolated_storage();
        let mut app = App::new(Vec::new());
        press(&mut app, KeyCode::Char('a'));
        assert!(matches!(press(&mut app, KeyCode::Char('q')), Action::None));
        let Some(Popup::Form(form)) = &app.popup else {
            panic!()
        };
        assert_eq!(form.fields[NAME], "q");
    }
}
