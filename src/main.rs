mod app;
mod db;
mod event;
mod logger;
mod model;
mod secret;
mod storage;
#[cfg(test)]
mod test_support;
mod ui;

use anyhow::Result;
use tokio::sync::mpsc;
use tuikit::term::{Input, Term};
use uuid::Uuid;

use app::App;
use event::Action;

/// Name of the data directory, log file and keychain service.
pub const APP: &str = "gridtui";

enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Paste(String),
    Tick,
    Resize,
    /// A connect attempt finished.
    Connected {
        id: Uuid,
        result: Result<db::Conn, String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Fails on a corrupt connections.json, before the terminal is taken over, so the
    // message stays readable.
    let (connections, imported) = match storage::load_connections()? {
        Some(c) => (c, 0),
        None => import_datagrust(),
    };
    let mut app = App::new(connections);
    if imported > 0 {
        app.status_message = format!("Imported {imported} connection(s) from datagrust");
    }

    let (mut terminal, keyboard_enhancement) = tuikit::term::setup()?;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let input_paused = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    tuikit::term::spawn_input(tx.clone(), input_paused, |input| match input {
        Input::Key(key) => AppEvent::Key(key),
        Input::Paste(text) => AppEvent::Paste(text),
        Input::Tick => AppEvent::Tick,
        Input::Resize => AppEvent::Resize,
    });

    let result = run(&mut terminal, &mut app, &tx, &mut rx).await;
    tuikit::term::restore(&mut terminal, keyboard_enhancement)?;
    result
}

async fn run(
    terminal: &mut Term,
    app: &mut App,
    tx: &mpsc::UnboundedSender<AppEvent>,
    rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    terminal.draw(|f| ui::draw(f, app))?;
    while let Some(evt) = rx.recv().await {
        match evt {
            AppEvent::Tick => continue,
            AppEvent::Resize => {}
            AppEvent::Key(key) => match event::handle_key(app, key) {
                Action::Quit => break,
                Action::Connect(conn) => spawn_connect(conn, tx.clone()),
                Action::None => {}
            },
            AppEvent::Paste(text) => event::handle_paste(app, text),
            AppEvent::Connected { id, result } => app.finish_connect(id, result),
        }
        terminal.draw(|f| ui::draw(f, app))?;
    }
    Ok(())
}

fn spawn_connect(conn: model::Connection, tx: mpsc::UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let id = conn.id;
        // The keychain call can block on an OS prompt; keep it off the runtime threads.
        let password = tokio::task::spawn_blocking(move || secret::get_password(id)).await;
        let result = match password {
            Ok(Ok(password)) => db::Conn::connect(&conn, &password).await,
            Ok(Err(e)) => Err(format!("Couldn't read the password from the keychain: {e}")),
            Err(e) => Err(format!("Keychain lookup failed: {e}")),
        };
        let _ = tx.send(AppEvent::Connected { id, result });
    });
}

/// First run: bring over datagrust's Postgres connections, moving their plaintext
/// passwords into the keychain. datagrust's own file is left untouched.
fn import_datagrust() -> (Vec<model::Connection>, usize) {
    let imported = storage::read_datagrust_connections();
    if imported.is_empty() {
        return (Vec::new(), 0);
    }
    let mut connections = Vec::new();
    for (conn, password) in imported {
        if let Err(e) = secret::set_password(conn.id, &password) {
            logger::log(&format!("import '{}' password", conn.name), &e.to_string());
        }
        connections.push(conn);
    }
    if let Err(e) = storage::save_connections(&connections) {
        logger::log("import save", &format!("{e:#}"));
    }
    let n = connections.len();
    (connections, n)
}
