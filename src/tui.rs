use crate::{config, db};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
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
use std::io;

const RESULT_LIMIT: usize = 20;

pub enum Action {
    Continue,
    Quit,
    Select(String),
}

#[derive(Default)]
pub struct App {
    pub input: String,
    pub results: Vec<db::CollapsedRecord>,
    pub selected: usize,
    pub error: Option<String>,
}

impl App {
    pub fn set_results(&mut self, results: Vec<db::CollapsedRecord>) {
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Esc => Action::Quit,
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Enter => self
                .results
                .get(self.selected)
                .map_or(Action::Continue, |r| Action::Select(r.command_text.clone())),
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                Action::Continue
            }
            KeyCode::Down => {
                if self.selected + 1 < self.results.len() {
                    self.selected += 1;
                }
                Action::Continue
            }
            KeyCode::Backspace => {
                self.input.pop();
                Action::Continue
            }
            KeyCode::Char(c) => {
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
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &connection);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    connection: &rusqlite::Connection,
) -> io::Result<String> {
    let mut app = App::default();
    let mut needs_query = false;

    loop {
        if needs_query {
            match db::filtered_collapsed_records(
                connection,
                if app.input.is_empty() {
                    None
                } else {
                    Some(&app.input)
                },
                &db::SearchFilters::default(),
                RESULT_LIMIT,
            ) {
                Ok(results) => app.set_results(results),
                Err(e) => app.error = Some(format!("Search error: {e}")),
            }
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
            match app.handle_key(key.code) {
                Action::Quit => return Ok(String::new()),
                Action::Select(text) => return Ok(text),
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
    } else if app.input.is_empty() {
        frame.render_widget(
            Paragraph::new("Type to search...")
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
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
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

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Results ({})", app.results.len())),
    );
    frame.render_widget(list, area);
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
        "cwd: {} | timestamp: {} | exit: {}{}",
        record.most_recent_cwd, record.most_recent_executed_at, exit, repeat
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
    ];

    let preview = Paragraph::new(Text::from(text)).block(Block::default().borders(Borders::ALL));
    frame.render_widget(preview, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[test]
    fn typing_adds_characters() {
        let mut app = App::default();
        app.handle_key(KeyCode::Char('d'));
        app.handle_key(KeyCode::Char('o'));
        app.handle_key(KeyCode::Char('c'));
        assert_eq!(app.input, "doc");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut app = App::default();
        app.handle_key(KeyCode::Char('a'));
        app.handle_key(KeyCode::Char('b'));
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.input, "a");
    }

    #[test]
    fn backspace_on_empty_input_does_nothing() {
        let mut app = App::default();
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.input, "");
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
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected, 1);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected, 1);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected, 0);
        app.handle_key(KeyCode::Up);
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

        match app.handle_key(KeyCode::Enter) {
            Action::Select(text) => assert_eq!(text, "cargo test"),
            _ => panic!("expected Select"),
        }
    }

    #[test]
    fn enter_on_empty_results_does_nothing() {
        let mut app = App::default();
        match app.handle_key(KeyCode::Enter) {
            Action::Continue => {}
            _ => panic!("expected Continue on empty results"),
        }
    }

    #[test]
    fn esc_quits() {
        let mut app = App::default();
        match app.handle_key(KeyCode::Esc) {
            Action::Quit => {}
            _ => panic!("expected Quit"),
        }
    }

    #[test]
    fn q_quits() {
        let mut app = App::default();
        match app.handle_key(KeyCode::Char('q')) {
            Action::Quit => {}
            _ => panic!("expected Quit"),
        }
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
        app.handle_key(KeyCode::Down);
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
}
