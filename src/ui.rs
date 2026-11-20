//! Rendering with ratatui; one `draw_*` function per pane or popup.
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, ConnForm, ConnState, FORM_LABELS, FORM_ROWS, PASSWORD, Popup, TLS};

const DIM: Style = Style::new().fg(Color::DarkGray);

pub fn draw(f: &mut Frame, app: &App) {
    let [main, status] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(f.area());
    let [sidebar, details] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Min(20)]).areas(main);
    draw_connections(f, app, sidebar);
    draw_details(f, app, details);
    f.render_widget(Paragraph::new(app.status_message.as_str()), status);
    match &app.popup {
        Some(Popup::Form(form)) => draw_form(f, form),
        Some(Popup::ConfirmDelete(id)) => {
            let name = app
                .connections
                .iter()
                .find(|c| c.id == *id)
                .map_or("", |c| c.name.as_str());
            draw_confirm(f, name);
        }
        None => {}
    }
}

fn state_icon(state: Option<&ConnState>) -> Span<'static> {
    match state {
        None => Span::styled("○", DIM),
        Some(ConnState::Connecting) => Span::styled("…", Style::new().fg(Color::Yellow)),
        Some(ConnState::Connected(_)) => Span::styled("●", Style::new().fg(Color::Green)),
        Some(ConnState::Failed(_)) => Span::styled("✗", Style::new().fg(Color::Red)),
    }
}

fn draw_connections(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Cyan))
        .title(" Connections ");
    let items: Vec<ListItem> = app
        .connections
        .iter()
        .map(|c| {
            ListItem::new(Line::from(vec![
                state_icon(app.states.get(&c.id)),
                Span::raw(format!(" {}", c.name)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default().with_selected(Some(app.selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).border_style(DIM);
    let hint = Line::styled(
        "a add · e edit · D delete · Enter connect · x disconnect · q quit",
        DIM,
    );
    let Some(c) = app.selected_connection() else {
        let text = vec![
            Line::raw("No connections yet. Press a to add one."),
            Line::raw(""),
            hint,
        ];
        f.render_widget(Paragraph::new(text).block(block), area);
        return;
    };
    let mut text = vec![
        Line::styled(c.name.clone(), Style::new().add_modifier(Modifier::BOLD)),
        Line::raw(c.summary()),
        Line::raw(format!("TLS: {}", c.ssl.label())),
        Line::raw(""),
    ];
    match app.states.get(&c.id) {
        None => text.push(Line::styled("Not connected", DIM)),
        Some(ConnState::Connecting) => text.push(Line::raw("Connecting...")),
        Some(ConnState::Connected(conn)) => text.push(Line::styled(
            format!("Connected: {}", conn.server_version()),
            Style::new().fg(Color::Green),
        )),
        Some(ConnState::Failed(e)) => text.push(Line::styled(
            format!("Connection failed: {e}"),
            Style::new().fg(Color::Red),
        )),
    }
    text.extend([Line::raw(""), hint]);
    f.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn draw_form(f: &mut Frame, form: &ConnForm) {
    const LABEL_WIDTH: u16 = 11;
    let area = centered_rect(64, FORM_ROWS as u16 + 6, f.area());
    f.render_widget(Clear, area);
    let title = if form.id.is_some() {
        " Edit connection "
    } else {
        " New connection "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Cyan))
        .title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    for (row, label) in FORM_LABELS.iter().enumerate() {
        let focused = form.focus == row;
        let value = if row == TLS {
            Span::raw(format!("{} (space to toggle)", form.ssl.label()))
        } else if row == PASSWORD && form.id.is_some() && !form.password_edited {
            Span::styled("(unchanged; type to replace)", DIM)
        } else if row == PASSWORD {
            Span::raw("•".repeat(form.fields[row].chars().count()))
        } else {
            Span::raw(form.fields[row].clone())
        };
        let label_style = if focused {
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            DIM
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{label:<width$}", width = LABEL_WIDTH as usize),
                label_style,
            ),
            value,
        ]));
        if focused && row != TLS {
            let typed = if row == PASSWORD && !form.password_edited && form.id.is_some() {
                0
            } else {
                form.fields[row].chars().count() as u16
            };
            let x = (inner.x + LABEL_WIDTH + typed).min(inner.right().saturating_sub(1));
            f.set_cursor_position((x, inner.y + row as u16));
        }
    }
    lines.push(Line::raw(""));
    if let Some(e) = &form.error {
        lines.push(Line::styled(e.clone(), Style::new().fg(Color::Red)));
    }
    lines.push(Line::styled("Tab next · Enter save · Esc cancel", DIM));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_confirm(f: &mut Frame, name: &str) {
    let area = centered_rect(56, 5, f.area());
    f.render_widget(Clear, area);
    let text = vec![
        Line::raw(format!("Delete '{name}' and its saved password?")),
        Line::styled("y delete · any other key cancels", DIM),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Red))
        .title(" Delete connection ");
    f.render_widget(Paragraph::new(text).block(block), area);
}
