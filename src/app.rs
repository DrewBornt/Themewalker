//! Application state and business logic.
//!
//! `App` owns the theme list, the current selection cursor, and the UI mode
//! (browsing vs. confirming a selection).  It exposes a `handle_key` method
//! that the event loop calls; that method returns `Some(ExitAction)` when the
//! loop should terminate.

use crossterm::event::KeyCode;
use ratatui::widgets::ListState;

use crate::config::SddmConfig;
use crate::theme::SddmTheme;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// What the TUI loop should do when it returns.
#[derive(Debug)]
pub enum ExitAction {
    /// User pressed `q` / `Esc` without selecting a theme.
    Quit,
    /// User confirmed a theme – call `SddmConfig::write_theme` with this name.
    ApplyTheme(String),
}

/// UI modes that drive which widgets are rendered and which keys are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal list navigation.
    Browsing,
    /// Floating confirmation dialog.
    Confirming,
}

/// Central application state.
pub struct App {
    /// All installed themes, sorted alphabetically.
    pub themes: Vec<SddmTheme>,
    /// ratatui list state (tracks scroll offset and selection highlight).
    pub list_state: ListState,
    /// Currently active theme name (from config).
    pub current_theme: Option<String>,
    /// Loaded configuration (used when writing back).
    pub config: SddmConfig,
    /// Current UI mode.
    pub mode: Mode,
    /// Non-fatal notice shown in the status bar (e.g. "No themes found").
    pub status: Option<String>,
}

impl App {
    /// Build the initial state.
    ///
    /// The list cursor is pre-positioned on the currently active theme when
    /// it can be found in the theme list; otherwise it starts at index 0.
    pub fn new(themes: Vec<SddmTheme>, config: SddmConfig) -> Self {
        let initial_selection = config
            .current_theme
            .as_deref()
            .and_then(|name| themes.iter().position(|t| t.name == name))
            .unwrap_or(0);

        let mut list_state = ListState::default();
        if !themes.is_empty() {
            list_state.select(Some(initial_selection));
        }

        let status = if themes.is_empty() {
            Some("No themes found in /usr/share/sddm/themes/".to_string())
        } else {
            None
        };

        Self {
            current_theme: config.current_theme.clone(),
            themes,
            list_state,
            config,
            mode: Mode::Browsing,
            status,
        }
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Index of the highlighted item (guaranteed valid when themes is non-empty).
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    /// The theme currently highlighted in the list.
    pub fn highlighted_theme(&self) -> Option<&SddmTheme> {
        self.selected_index().and_then(|i| self.themes.get(i))
    }

    // -----------------------------------------------------------------------
    // Key handling (called by the event loop)
    // -----------------------------------------------------------------------

    /// Process a key press.  Returns `Some(ExitAction)` to signal the event
    /// loop to break; returns `None` to continue.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<ExitAction> {
        match self.mode {
            Mode::Browsing => self.handle_browsing_key(code),
            Mode::Confirming => self.handle_confirming_key(code),
        }
    }

    fn handle_browsing_key(&mut self, code: KeyCode) -> Option<ExitAction> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
                None
            }
            KeyCode::Enter => {
                if self.themes.is_empty() {
                    None
                } else {
                    self.mode = Mode::Confirming;
                    None
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => Some(ExitAction::Quit),
            _ => None,
        }
    }

    fn handle_confirming_key(&mut self, code: KeyCode) -> Option<ExitAction> {
        match code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                let theme_name = self
                    .highlighted_theme()
                    .map(|t| t.name.clone())
                    .expect("Confirming mode requires a selected theme");
                Some(ExitAction::ApplyTheme(theme_name))
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = Mode::Browsing;
                None
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Cursor movement
    // -----------------------------------------------------------------------

    fn move_up(&mut self) {
        if self.themes.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(0) | None => self.themes.len() - 1, // wrap to bottom
            Some(i) => i - 1,
        };
        self.list_state.select(Some(next));
    }

    fn move_down(&mut self) {
        if self.themes.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            None => 0,
            Some(i) => (i + 1) % self.themes.len(), // wrap to top
        };
        self.list_state.select(Some(next));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::SddmTheme;
    use std::path::PathBuf;

    fn make_theme(name: &str) -> SddmTheme {
        SddmTheme {
            name: name.to_string(),
            path: PathBuf::from("/tmp"),
            description: None,
            author: None,
        }
    }

    fn make_app(names: &[&str], current: Option<&str>) -> App {
        let themes: Vec<SddmTheme> = names.iter().map(|n| make_theme(n)).collect();
        let config = SddmConfig::empty();
        let mut app = App::new(themes, config);
        // Override current_theme for test convenience
        app.current_theme = current.map(|s| s.to_string());
        app
    }

    #[test]
    fn initial_selection_starts_at_zero_when_no_current() {
        let app = make_app(&["alpha", "beta", "gamma"], None);
        assert_eq!(app.selected_index(), Some(0));
    }

    #[test]
    fn initial_selection_preselects_current_theme() {
        let themes = vec![make_theme("alpha"), make_theme("beta"), make_theme("gamma")];
        let mut config = SddmConfig::empty();
        config.current_theme = Some("beta".to_string());
        let app = App::new(themes, config);
        assert_eq!(app.selected_index(), Some(1));
    }

    #[test]
    fn move_down_wraps_at_end() {
        let mut app = make_app(&["a", "b", "c"], None);
        app.list_state.select(Some(2));
        app.move_down();
        assert_eq!(app.selected_index(), Some(0));
    }

    #[test]
    fn move_up_wraps_at_start() {
        let mut app = make_app(&["a", "b", "c"], None);
        app.list_state.select(Some(0));
        app.move_up();
        assert_eq!(app.selected_index(), Some(2));
    }

    #[test]
    fn enter_switches_to_confirming_mode() {
        let mut app = make_app(&["alpha"], None);
        let result = app.handle_key(KeyCode::Enter);
        assert!(result.is_none());
        assert_eq!(app.mode, Mode::Confirming);
    }

    #[test]
    fn confirming_enter_returns_apply_action() {
        let mut app = make_app(&["alpha"], None);
        app.mode = Mode::Confirming;
        let result = app.handle_key(KeyCode::Enter);
        assert!(matches!(result, Some(ExitAction::ApplyTheme(ref n)) if n == "alpha"));
    }

    #[test]
    fn confirming_esc_returns_to_browsing() {
        let mut app = make_app(&["alpha"], None);
        app.mode = Mode::Confirming;
        let result = app.handle_key(KeyCode::Esc);
        assert!(result.is_none());
        assert_eq!(app.mode, Mode::Browsing);
    }

    #[test]
    fn quit_key_returns_quit_action() {
        let mut app = make_app(&["alpha"], None);
        let result = app.handle_key(KeyCode::Char('q'));
        assert!(matches!(result, Some(ExitAction::Quit)));
    }

    #[test]
    fn empty_theme_list_has_no_selection() {
        let app = make_app(&[], None);
        assert_eq!(app.selected_index(), None);
    }
}
