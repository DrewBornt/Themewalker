//! TUI rendering.
//!
//! Layout (vertical split):
//!
//! ```text
//! ┌─ Themewalker Theme Changer ────────────────────┐
//! │ Config: /etc/sddm.conf  │  Current: breeze      │  ← header (3 rows)
//! └────────────────────────────────────────────────-┘
//! ┌─ Installed Themes (4 found) ───────────────────┐
//! │ >> breeze                      [active]         │  ← list (fills)
//! │    maya                                         │
//! │    sugar-candy                                  │
//! └─────────────────────────────────────────────────┘
//! ┌─────────────────────────────────────────────────┐
//! │  ↑/↓ k/j  Navigate   Enter  Select   q  Quit   │  ← help bar (3 rows)
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! When `app.mode == Mode::Confirming` a centred popup overlays the list.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, Mode};

// ---------------------------------------------------------------------------
// Colour palette
// ---------------------------------------------------------------------------

const CLR_HIGHLIGHT_BG: Color = Color::Blue;
const CLR_HIGHLIGHT_FG: Color = Color::White;
const CLR_ACTIVE_BADGE: Color = Color::Green;
const CLR_HEADER_TITLE: Color = Color::Cyan;
const CLR_HELP_KEY: Color = Color::Yellow;
const CLR_POPUP_BORDER: Color = Color::LightYellow;
const CLR_POPUP_CONFIRM: Color = Color::LightGreen;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Draw the entire UI for one frame.  Takes `&mut App` because ratatui's
/// `render_stateful_widget` needs mutable access to `app.list_state`.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Three vertical bands: header | list | help
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);
    draw_theme_list(frame, app, chunks[1]);
    draw_help_bar(frame, app, chunks[2]);

    // Overlay the confirmation dialog on top of everything
    if app.mode == Mode::Confirming {
        draw_confirmation(frame, app, area);
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let current_label = app
        .current_theme
        .as_deref()
        .map(|n| format!("  Current: {}", n))
        .unwrap_or_else(|| "  Current: (unknown)".to_string());

    let config_label = format!("  Config: {}", app.config.path.display());

    let content = Line::from(vec![
        Span::styled(config_label, Style::default().fg(Color::DarkGray)),
        Span::raw("   "),
        Span::styled(current_label, Style::default().fg(CLR_ACTIVE_BADGE).add_modifier(Modifier::BOLD)),
    ]);

    let para = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " Themewalker Theme Changer ",
                    Style::default()
                        .fg(CLR_HEADER_TITLE)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .alignment(Alignment::Left);

    frame.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// Theme list
// ---------------------------------------------------------------------------

fn draw_theme_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let current = app.current_theme.as_deref().unwrap_or("");

    let items: Vec<ListItem> = app
        .themes
        .iter()
        .map(|theme| {
            if theme.name == current {
                ListItem::new(Line::from(vec![
                    Span::raw(pad_right(&theme.display_label(), 38)),
                    Span::styled(
                        "[active]",
                        Style::default()
                            .fg(CLR_ACTIVE_BADGE)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
            } else {
                ListItem::new(Span::raw(theme.display_label()))
            }
        })
        .collect();

    let title = if items.is_empty() {
        " Installed Themes ".to_string()
    } else {
        format!(" Installed Themes ({} found) ", items.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(CLR_HIGHLIGHT_BG)
                .fg(CLR_HIGHLIGHT_FG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // Status message when there are no themes
    if app.themes.is_empty() {
        let msg = app.status.as_deref().unwrap_or("No themes found.");
        let para = Paragraph::new(msg)
            .block(Block::default().borders(Borders::ALL).title(" Installed Themes "))
            .alignment(Alignment::Center);
        frame.render_widget(para, area);
    } else {
        frame.render_stateful_widget(list, area, &mut app.list_state);
    }
}

// ---------------------------------------------------------------------------
// Help bar
// ---------------------------------------------------------------------------

fn draw_help_bar(frame: &mut Frame, _app: &App, area: Rect) {
    let keys: &[(&str, &str)] = &[
        ("↑/↓ k/j", "Navigate"),
        ("Enter", "Select"),
        ("q / Esc", "Quit"),
    ];

    let mut spans = Vec::new();
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(
            format!("[{}]", key),
            Style::default().fg(CLR_HELP_KEY).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}", desc)));
    }

    let para = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    frame.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// Confirmation popup
// ---------------------------------------------------------------------------

fn draw_confirmation(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.highlighted_theme();
    let theme_name = theme.map(|t| t.name.as_str()).unwrap_or("?");
    let author_line = theme
        .and_then(|t| t.author.as_deref())
        .map(|a| format!("  by {a}"))
        .unwrap_or_default();

    // Popup is 54 columns wide, 10 rows tall
    let popup_area = centered_rect(54, 10, area);

    // Clear background so the popup isn't see-through
    frame.render_widget(Clear, popup_area);

    let mut body = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Apply theme  "),
            Span::styled(
                theme_name,
                Style::default()
                    .fg(CLR_POPUP_CONFIRM)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  ?"),
        ]),
    ];

    if !author_line.is_empty() {
        body.push(Line::from(Span::styled(
            author_line,
            Style::default().fg(Color::DarkGray),
        )));
    }

    body.extend([
        Line::from(""),
        Line::from(Span::styled(
            "  [Enter / y]  Confirm",
            Style::default().fg(CLR_HELP_KEY),
        )),
        Line::from(Span::styled(
            "  [Esc   / n]  Cancel",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  (sudo may be required to write config)",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )),
    ]);

    let popup = Paragraph::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(CLR_POPUP_BORDER))
                .title(Span::styled(
                    " Confirm ",
                    Style::default()
                        .fg(CLR_POPUP_BORDER)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .alignment(Alignment::Left);

    frame.render_widget(popup, popup_area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a Rect centred within `r` with the given fixed width and height.
fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x.saturating_add(r.width.saturating_sub(width) / 2);
    let y = r.y.saturating_add(r.height.saturating_sub(height) / 2);
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect::new(x, y, w, h)
}

/// Right-pad a string to at least `len` characters (for column alignment).
fn pad_right(s: &str, len: usize) -> String {
    if s.len() >= len {
        s.to_string()
    } else {
        format!("{:<width$}", s, width = len)
    }
}
