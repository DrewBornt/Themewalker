//! `themewalker` — SDDM theme changer TUI
//!
//! # Execution flow
//!
//! 1. Load SDDM config (best-effort; falls back to empty state).
//! 2. Discover installed themes under `/usr/share/sddm/themes/`.
//! 3. Install a panic hook that restores the terminal before printing.
//! 4. Enter alternate-screen raw mode and run the ratatui event loop.
//! 5. On exit, restore the terminal unconditionally.
//! 6. If the user confirmed a theme, write it to the config file
//!    (using `sudo tee` when the current process lacks write permission).

mod app;
mod config;
mod theme;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, ExitAction};
use config::SddmConfig;
use theme::discover_themes;

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    // ------------------------------------------------------------------
    // 1. Load config (non-fatal: fall back to empty)
    // ------------------------------------------------------------------
    let config = match SddmConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: could not read SDDM config ({e}); starting with empty state.");
            SddmConfig::empty()
        }
    };

    // ------------------------------------------------------------------
    // 2. Discover themes
    // ------------------------------------------------------------------
    let themes = discover_themes().context("Failed to scan theme directory")?;

    // ------------------------------------------------------------------
    // 3. Build app state
    // ------------------------------------------------------------------
    let mut app = App::new(themes, config);

    // ------------------------------------------------------------------
    // 4. Panic hook – restore terminal so the panic message is readable
    // ------------------------------------------------------------------
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal_raw();
        original_hook(info);
    }));

    // ------------------------------------------------------------------
    // 5. Enter the TUI
    // ------------------------------------------------------------------
    let mut terminal = enter_terminal()?;
    let result = run_event_loop(&mut terminal, &mut app);

    // ------------------------------------------------------------------
    // 6. Restore terminal (always – even on error)
    // ------------------------------------------------------------------
    let restore_err = restore_terminal(&mut terminal);

    // Propagate event-loop error before restore error
    let action = result?;
    restore_err?;

    // ------------------------------------------------------------------
    // 7. Apply selected theme (post-TUI, in normal terminal mode)
    // ------------------------------------------------------------------
    match action {
        ExitAction::Quit => {}
        ExitAction::ApplyTheme(ref name) => {
            println!("Applying theme '{name}'…");
            println!("Config path: {}", app.config.path.display());
            match app.config.write_theme(name) {
                Ok(()) => {
                    println!("Done.  Restart SDDM (or log out) for the change to take effect.");
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Terminal setup / teardown
// ---------------------------------------------------------------------------

fn enter_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("Failed to create ratatui terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;
    Ok(())
}

/// Used only by the panic hook (no terminal handle available there).
fn restore_terminal_raw() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

/// Render frames and dispatch key events until the user picks an action.
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<ExitAction> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Poll with a short timeout so we can keep re-drawing on resize, etc.
        if event::poll(Duration::from_millis(200))? {
            let ev = event::read()?;

            // Only react to actual key presses (ignore key-release on Windows)
            if let Event::Key(key) = ev {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(action) = app.handle_key(key.code) {
                    return Ok(action);
                }
            }

            // Re-render immediately on terminal resize
            if let Event::Resize(_, _) = ev {
                terminal.autoresize()?;
            }
        }
    }
}
