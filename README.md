# gridtui

A keyboard-driven terminal database client: a lightweight DataGrip
alternative, built on the same stack as
[postui](https://github.com/huyvu8051/postui) (ratatui, tokio, and the shared
[tuikit](https://github.com/huyvu8051/tuikit)).

Early days: gridtui can save PostgreSQL connections and connect to them.
Browsing schemas, a query console and the results grid come next.

## Install

Requires Rust 1.88 or newer.

```bash
cargo install --git https://github.com/huyvu8051/gridtui --locked
gridtui
```

## Connections

| Key | Action |
|---|---|
| `a` | Add a connection |
| `e` | Edit the selected connection |
| `D` | Delete it (asks first) |
| `Enter` | Connect |
| `x` | Disconnect |
| `j` / `k` | Move |
| `q`, `Ctrl+C` | Quit |

In the form: `Tab` / `Shift+Tab` move between fields, `Space` toggles TLS,
`Enter` saves, `Esc` cancels. When editing, the password field stays as saved
unless you type in it.

- **Passwords** are kept in the OS keychain (service `gridtui`, one entry per
  connection), never in files.
- **Connections** are saved to `connections.json` in the data directory
  (`~/Library/Application Support/gridtui` on macOS, `~/.local/share/gridtui`
  on Linux), next to `gridtui.log`, where connection errors are logged.
- **TLS**: `disable` or `verify-full` (certificate checked against the OS trust
  store).
- **From datagrust**: on the first start, Postgres connections saved by
  datagrust (`~/.config/datagrust/connections.toml`) are imported and their
  passwords moved into the keychain. datagrust's file is left as it was.

## Architecture

Same layout as postui:

| Module | Responsibility |
|---|---|
| `main.rs` | Terminal setup (tuikit), event loop, spawning connects |
| `app.rs` | `App`: all UI state and the operations on it; the connection form |
| `event.rs` | Key routing: open popup first, then the connection list |
| `ui.rs` | Rendering; one `draw_*` function per pane or popup |
| `model.rs` | Serializable data (`Connection`, `SslMode`) |
| `storage.rs` | `connections.json`, datagrust import |
| `secret.rs` | Keychain access |
| `db/` | Database access. Only `db/` uses a driver crate; `db::Conn` is an enum with one variant per database (`Pg` today). Adding a database means a module next to `db/pg.rs` and a `Conn` variant. |

## Development

```bash
cargo test
scripts/dev-postgres.sh up     # disposable Postgres on :5599 (needs initdb/pg_ctl)
scripts/dev-postgres.sh dsn    # host=127.0.0.1 port=5599 user=gridtui dbname=gridtui_dev
scripts/dev-postgres.sh down
```

Tests run against a throwaway `HOME` and keyring's in-memory mock, never your
real data or keychain (`test_support::isolated_storage()`).

## License

MIT OR Apache-2.0.
