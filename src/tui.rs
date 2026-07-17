use crate::{config, db, format_relative_timestamp};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, Write};
use std::process::{Command, Stdio};

const RESULT_LIMIT: usize = 20;
const RECENT_LIMIT: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
    Copy(String),
    Insert(String),
    Rerun(String),
}

#[derive(Default)]
pub struct App {
    pub input: String,
    pub results: Vec<db::CollapsedRecord>,
    pub selected: usize,
    pub error: Option<String>,
    pub status: Option<String>,
}

impl App {
    pub fn set_results(&mut self, results: Vec<db::CollapsedRecord>) {
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        let selected = || {
            self.results
                .get(self.selected)
                .map(|r| r.command_text.clone())
        };
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => Action::Quit,
            (KeyCode::Enter, _) => selected().map_or(Action::Continue, Action::Insert),
            (KeyCode::Char('y'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                selected().map_or(Action::Continue, Action::Copy)
            }
            (KeyCode::F(5), _) => selected().map_or(Action::Continue, Action::Rerun),
            (KeyCode::Up, _) => {
                self.selected = self.selected.saturating_sub(1);
                Action::Continue
            }
            (KeyCode::Down, _) => {
                if self.selected + 1 < self.results.len() {
                    self.selected += 1;
                }
                Action::Continue
            }
            (KeyCode::Backspace, _) => {
                self.input.pop();
                Action::Continue
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.input.push(c);
                Action::Continue
            }
            _ => Action::Continue,
        }
    }
}

pub fn run() -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;

    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &connection);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    connection: &rusqlite::Connection,
) -> io::Result<String> {
    let mut app = App::default();
    refresh_results(&mut app, connection);
    let mut needs_query = false;

    loop {
        if needs_query {
            refresh_results(&mut app, connection);
            needs_query = false;
        }

        terminal.draw(|frame| render(frame, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(String::new());
            }
            let previous_input = app.input.clone();
            match app.handle_key(key) {
                Action::Quit => return Ok(String::new()),
                Action::Insert(text) => return Ok(shell_output("insert", &text)),
                Action::Rerun(text) => return Ok(shell_output("rerun", &text)),
                Action::Copy(text) => {
                    app.status = Some(match copy_to_clipboard(&text) {
                        Ok(()) => "Copied raw command to clipboard.".to_string(),
                        Err(error) => format!("Copy unavailable: {error}"),
                    });
                }
                Action::Continue => {
                    if app.input != previous_input {
                        needs_query = true;
                        app.error = None;
                    }
                }
            }
        }
    }
}

fn refresh_results(app: &mut App, connection: &rusqlite::Connection) {
    let results = if app.input.is_empty() {
        db::recent_collapsed_records(connection, &db::SearchFilters::default(), RECENT_LIMIT)
    } else {
        db::fuzzy_filtered_collapsed_records(
            connection,
            &app.input,
            &db::SearchFilters::default(),
            RESULT_LIMIT,
        )
    };
    match results {
        Ok(results) => app.set_results(results),
        Err(error) => app.error = Some(format!("Search error: {error}")),
    }
}

fn shell_output(action: &str, text: &str) -> String {
    format!("{action}\n{text}")
}

fn copy_to_clipboard(text: &str) -> io::Result<()> {
    let providers: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[("clip.exe", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };

    copy_with_providers(text, providers)
}

fn copy_with_providers(text: &str, providers: &[(&str, &[&str])]) -> io::Result<()> {
    for (program, args) in providers {
        let Ok(mut child) = Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .spawn()
        else {
            continue;
        };
        let wrote = child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(text.as_bytes())
            .is_ok();
        let status = child.wait();
        if wrote && status.is_ok_and(|status| status.success()) {
            return Ok(());
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no supported clipboard provider found",
    ))
}

fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(8),
        ])
        .split(frame.area());

    render_search_bar(frame, chunks[0], app);

    if let Some(error) = &app.error {
        frame.render_widget(
            Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL).title("Error")),
            chunks[1],
        );
    } else if app.results.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.input.is_empty() {
                "No command history yet — type to search after running commands."
            } else {
                "No matching commands."
            })
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Results")),
            chunks[1],
        );
    } else {
        render_results(frame, chunks[1], app);
    }

    if app.selected < app.results.len() {
        render_preview(frame, chunks[2], app);
    }
}

fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Search"))
        .style(Style::default().fg(Color::White));
    frame.render_widget(input, area);
    frame.set_cursor_position((area.x + 1 + app.input.len() as u16, area.y + 1));
}

fn render_results(frame: &mut Frame, area: Rect, app: &App) {
    let visible_rows = area.height.saturating_sub(2) as usize;
    let start = visible_result_start(app.selected, visible_rows);
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .skip(start)
        .take(visible_rows)
        .map(|(i, record)| {
            let is_selected = i == app.selected;
            let prefix = if is_selected { "> " } else { "  " };
            let mut style = Style::default().fg(if is_selected {
                Color::Yellow
            } else {
                Color::White
            });
            if is_selected {
                style = style.add_modifier(Modifier::BOLD);
            }

            let repeat = if record.repeat_count > 1 {
                format!("  (x{})", record.repeat_count)
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{prefix}{}", record.command_text), style),
                Span::styled(repeat, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let title = if app.input.is_empty() {
        format!("Recent commands ({}) — type to search", app.results.len())
    } else {
        format!("Results ({})", app.results.len())
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(list, area);
}

fn visible_result_start(selected: usize, visible_rows: usize) -> usize {
    selected.saturating_sub(visible_rows.saturating_sub(1))
}

fn render_preview(frame: &mut Frame, area: Rect, app: &App) {
    let record = &app.results[app.selected];

    let exit = if record.all_same_exit {
        format!("{}", record.most_recent_exit_code)
    } else {
        format!("mixed (most recent: {})", record.most_recent_exit_code)
    };

    let mut repeat = String::new();
    if record.repeat_count > 1 {
        repeat = format!(" | repeats: {}", record.repeat_count);
    }

    let mut meta = format!(
        "cwd: {} | last used: {} | exit: {}{}",
        record.most_recent_cwd,
        format_relative_timestamp(record.most_recent_executed_at),
        exit,
        repeat
    );
    if let Some(repo) = &record.repo {
        meta.push_str(&format!(" | repo: {repo}"));
    }
    if let Some(branch) = &record.branch {
        meta.push_str(&format!(" | branch: {branch}"));
    }

    let text = vec![
        Line::from(vec![Span::styled(
            "Preview",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            &record.command_text,
            Style::default().fg(Color::Green),
        )]),
        Line::from(vec![Span::styled(meta, Style::default().fg(Color::Gray))]),
        Line::from("[Enter] Insert/edit  [Ctrl+Y] Copy  [F5] Rerun (executes)"),
        Line::from(app.status.as_deref().unwrap_or("")),
    ];

    let preview = Paragraph::new(Text::from(text)).block(Block::default().borders(Borders::ALL));
    frame.render_widget(preview, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn typing_adds_characters() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Char('d')));
        app.handle_key(key(KeyCode::Char('o')));
        app.handle_key(key(KeyCode::Char('c')));
        assert_eq!(app.input, "doc");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Char('b')));
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.input, "a");
    }

    #[test]
    fn backspace_on_empty_input_does_nothing() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.input, "");
    }

    #[test]
    fn clearing_input_keeps_results_until_the_next_refresh() {
        let mut app = App {
            input: "a".into(),
            results: vec![record("cargo test")],
            ..App::default()
        };

        app.handle_key(key(KeyCode::Backspace));

        assert_eq!(app.results.len(), 1);
    }

    #[test]
    fn result_window_keeps_selection_visible() {
        assert_eq!(visible_result_start(2, 5), 0);
        assert_eq!(visible_result_start(7, 5), 3);
    }

    #[test]
    fn up_and_down_navigate_results() {
        let mut app = App::default();
        app.set_results(vec![
            db::CollapsedRecord {
                command_text: "cargo test".into(),
                normalized_text: "cargo test".into(),
                repeat_count: 1,
                most_recent_executed_at: 1,
                most_recent_cwd: "/tmp".into(),
                most_recent_exit_code: 0,
                all_same_exit: true,
                repo: None,
                branch: None,
            },
            db::CollapsedRecord {
                command_text: "cargo build".into(),
                normalized_text: "cargo build".into(),
                repeat_count: 1,
                most_recent_executed_at: 2,
                most_recent_cwd: "/tmp".into(),
                most_recent_exit_code: 0,
                all_same_exit: true,
                repo: None,
                branch: None,
            },
        ]);

        assert_eq!(app.selected, 0);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected, 1);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected, 1);
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.selected, 0);
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn enter_selects_current_result() {
        let mut app = App::default();
        app.set_results(vec![db::CollapsedRecord {
            command_text: "cargo test".into(),
            normalized_text: "cargo test".into(),
            repeat_count: 1,
            most_recent_executed_at: 1,
            most_recent_cwd: "/tmp".into(),
            most_recent_exit_code: 0,
            all_same_exit: true,
            repo: None,
            branch: None,
        }]);

        match app.handle_key(key(KeyCode::Enter)) {
            Action::Insert(text) => assert_eq!(text, "cargo test"),
            _ => panic!("expected Insert"),
        }
    }

    #[test]
    fn enter_on_empty_results_does_nothing() {
        let mut app = App::default();
        match app.handle_key(key(KeyCode::Enter)) {
            Action::Continue => {}
            _ => panic!("expected Continue on empty results"),
        }
    }

    #[test]
    fn esc_quits() {
        let mut app = App::default();
        match app.handle_key(key(KeyCode::Esc)) {
            Action::Quit => {}
            _ => panic!("expected Quit"),
        }
    }

    #[test]
    fn q_is_typed() {
        let mut app = App::default();
        assert!(matches!(
            app.handle_key(key(KeyCode::Char('q'))),
            Action::Continue
        ));
        assert_eq!(app.input, "q");
    }

    #[test]
    fn selected_resets_when_results_shrink() {
        let mut app = App::default();
        app.set_results(vec![
            db::CollapsedRecord {
                command_text: "a".into(),
                normalized_text: "a".into(),
                repeat_count: 1,
                most_recent_executed_at: 1,
                most_recent_cwd: "/tmp".into(),
                most_recent_exit_code: 0,
                all_same_exit: true,
                repo: None,
                branch: None,
            },
            db::CollapsedRecord {
                command_text: "b".into(),
                normalized_text: "b".into(),
                repeat_count: 1,
                most_recent_executed_at: 2,
                most_recent_cwd: "/tmp".into(),
                most_recent_exit_code: 0,
                all_same_exit: true,
                repo: None,
                branch: None,
            },
        ]);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected, 1);
        app.set_results(vec![db::CollapsedRecord {
            command_text: "a".into(),
            normalized_text: "a".into(),
            repeat_count: 1,
            most_recent_executed_at: 1,
            most_recent_cwd: "/tmp".into(),
            most_recent_exit_code: 0,
            all_same_exit: true,
            repo: None,
            branch: None,
        }]);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn copy_and_rerun_are_explicit_actions_on_raw_text() {
        let mut app = App::default();
        app.set_results(vec![record("cargo   test -- --exact")]);

        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
            Action::Copy("cargo   test -- --exact".into())
        );
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
            Action::Rerun("cargo   test -- --exact".into())
        );
    }

    #[test]
    fn terminal_portable_shortcuts_avoid_line_editing_collisions() {
        let mut app = App::default();
        app.set_results(vec![record("cargo test")]);

        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
            Action::Copy("cargo test".into())
        );
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
            Action::Rerun("cargo test".into())
        );
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            Action::Continue
        );
    }

    #[test]
    fn empty_search_shows_recent_commands_and_hint() {
        let app = App {
            results: vec![record("docker compose up")],
            ..App::default()
        };
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");

        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("docker compose up"));
        assert!(rendered.contains("Recent commands"));
        assert!(rendered.contains("type to search"));
    }

    #[test]
    fn shell_output_preserves_raw_multiline_command() {
        assert_eq!(
            shell_output("insert", "printf 'one  two'\nprintf three"),
            "insert\nprintf 'one  two'\nprintf three"
        );
        assert_eq!(shell_output("rerun", "cargo test"), "rerun\ncargo test");
    }

    #[cfg(unix)]
    #[test]
    fn clipboard_falls_back_after_provider_write_failure() {
        let root = std::env::temp_dir().join(format!(
            "seekr-clipboard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp directory should be created");
        let output = root.join("clipboard");
        let script = format!("cat > '{}'", output.display());
        let large_text = "clipboard fallback".repeat(100_000);

        copy_with_providers(
            &large_text,
            &[("false", &[]), ("sh", &["-c", script.as_str()])],
        )
        .expect("second provider should succeed");

        assert_eq!(
            std::fs::read_to_string(output).expect("clipboard output should exist"),
            large_text
        );
        std::fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    fn record(command_text: &str) -> db::CollapsedRecord {
        db::CollapsedRecord {
            command_text: command_text.into(),
            normalized_text: command_text.into(),
            repeat_count: 1,
            most_recent_executed_at: 1,
            most_recent_cwd: "/tmp".into(),
            most_recent_exit_code: 0,
            all_same_exit: true,
            repo: None,
            branch: None,
        }
    }
}
