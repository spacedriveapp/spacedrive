//! Terminal UI components using ratatui

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::{
    collections::HashMap,
    io,
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::colors::{job_status_color, job_status_icon};
use sd_core::{infra::job::types::JobStatus, ops::jobs::list::output::JobListItem};

/// Job monitor TUI state
pub struct JobMonitorTui {
    pub jobs: Vec<JobListItem>,
    pub selected_job: Option<usize>,
    pub list_state: ListState,
    pub last_update: Instant,
    pub spinner_frame: usize,
    pub show_help: bool,
    pub filter_status: Option<JobStatus>,
}

impl JobMonitorTui {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            selected_job: None,
            list_state: ListState::default(),
            last_update: Instant::now(),
            spinner_frame: 0,
            show_help: false,
            filter_status: None,
        }
    }

    /// Update jobs list
    pub fn update_jobs(&mut self, jobs: Vec<JobListItem>) {
        self.jobs = jobs;
        self.last_update = Instant::now();

        // Update selection if needed
        if let Some(selected) = self.selected_job {
            if selected >= self.jobs.len() {
                self.selected_job = if self.jobs.is_empty() { None } else { Some(0) };
            }
        }

        // Update list state
        if let Some(selected) = self.selected_job {
            self.list_state.select(Some(selected));
        }
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.jobs.is_empty() {
            return;
        }

        let selected = match self.selected_job {
            Some(i) => {
                if i == 0 {
                    self.jobs.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };

        self.selected_job = Some(selected);
        self.list_state.select(Some(selected));
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.jobs.is_empty() {
            return;
        }

        let selected = match self.selected_job {
            Some(i) => {
                if i >= self.jobs.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        self.selected_job = Some(selected);
        self.list_state.select(Some(selected));
    }

    /// Get selected job
    pub fn get_selected_job(&self) -> Option<&JobListItem> {
        self.selected_job.and_then(|i| self.jobs.get(i))
    }

    /// Toggle help display
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Cycle through status filters
    pub fn cycle_filter(&mut self) {
        self.filter_status = match self.filter_status {
            None => Some(JobStatus::Running),
            Some(JobStatus::Running) => Some(JobStatus::Completed),
            Some(JobStatus::Completed) => Some(JobStatus::Failed),
            Some(JobStatus::Failed) => None,
            _ => None,
        };
    }

    /// Get filtered jobs
    pub fn get_filtered_jobs(&self) -> Vec<&JobListItem> {
        match self.filter_status {
            Some(status) => self.jobs.iter().filter(|job| job.status == status).collect(),
            None => self.jobs.iter().collect(),
        }
    }

    /// Tick animation
    pub fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 10;
    }
}

/// Render the job monitor TUI
pub fn render_job_monitor<B: Backend>(f: &mut Frame<B>, app: &mut JobMonitorTui) {
    let size = f.size();

    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer
        ])
        .split(size);

    // Render header
    render_header(f, chunks[0], app);

    // Render main content based on state
    if app.show_help {
        render_help(f, chunks[1]);
    } else {
        render_job_list(f, chunks[1], app);
    }

    // Render footer
    render_footer(f, chunks[2], app);
}

/// Render the header section
fn render_header<B: Backend>(f: &mut Frame<B>, area: Rect, app: &JobMonitorTui) {
    let spinner_char = super::colors::spinner_char(app.spinner_frame);

    let title = format!(
        "{} Spacedrive Job Monitor - {} jobs",
        spinner_char,
        app.jobs.len()
    );

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
        );

    f.render_widget(header, area);
}

/// Render the job list
fn render_job_list<B: Backend>(f: &mut Frame<B>, area: Rect, app: &mut JobMonitorTui) {
    let jobs = app.get_filtered_jobs();

    // Create job list items
    let items: Vec<ListItem> = jobs
        .iter()
        .map(|job| {
            let progress_bar = create_progress_bar(job.progress, 20);
            let status_color = job_status_color(job.status);
            let status_icon = job_status_icon(job.status);

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color)
                ),
                Span::styled(
                    format!("{:<30}", job.name),
                    Style::default().fg(Color::White)
                ),
                Span::styled(
                    format!(" {} ", progress_bar),
                    Style::default()
                ),
                Span::styled(
                    format!("{:>6.1}%", job.progress * 100.0),
                    Style::default().fg(Color::Yellow)
                ),
                Span::styled(
                    format!(" [{}]", job.id.to_string()[..8].to_string()),
                    Style::default().fg(Color::DarkGray)
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    // Create filter info
    let filter_text = match app.filter_status {
        Some(status) => format!(" (filtered: {})", status),
        None => String::new(),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Jobs{}", filter_text))
                .border_style(Style::default().fg(Color::Blue))
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol("► ");

    f.render_stateful_widget(list, area, &mut app.list_state);

    // Render job details if one is selected
    if let Some(job) = app.get_selected_job() {
        render_job_details(f, area, job);
    }
}

/// Render job details popup
fn render_job_details<B: Backend>(f: &mut Frame<B>, area: Rect, job: &JobListItem) {
    let popup_area = centered_rect(60, 40, area);

    f.render_widget(Clear, popup_area);

    let details = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(job.id.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(job.name.clone()),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} {}", job_status_icon(job.status), job.status),
                Style::default().fg(job_status_color(job.status))
            ),
        ]),
        Line::from(vec![
            Span::styled("Progress: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:.1}%", job.progress * 100.0)),
        ]),
    ];

    let paragraph = Paragraph::new(details)
        .block(
            Block::default()
                .title("Job Details")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, popup_area);
}

/// Render help screen
fn render_help<B: Backend>(f: &mut Frame<B>, area: Rect) {
    let help_text = vec![
        Line::from("Keyboard Shortcuts:"),
        Line::from(""),
        Line::from("  ↑/k     - Move up"),
        Line::from("  ↓/j     - Move down"),
        Line::from("  Enter   - Show job details"),
        Line::from("  f       - Cycle status filter"),
        Line::from("  r       - Refresh jobs"),
        Line::from("  c       - Clear completed jobs"),
        Line::from("  h/?     - Toggle this help"),
        Line::from("  q/Esc   - Quit"),
        Line::from(""),
        Line::from("Job Status Icons:"),
        Line::from("  ⏳ Queued    ⚡ Running    ⏸️ Paused"),
        Line::from("  Completed ❌ Failed    🚫 Cancelled"),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
        )
        .alignment(Alignment::Left);

    f.render_widget(help, area);
}

/// Render the footer section
fn render_footer<B: Backend>(f: &mut Frame<B>, area: Rect, app: &JobMonitorTui) {
    let running_count = app.jobs.iter().filter(|j| j.status == JobStatus::Running).count();
    let completed_count = app.jobs.iter().filter(|j| j.status == JobStatus::Completed).count();
    let failed_count = app.jobs.iter().filter(|j| j.status == JobStatus::Failed).count();

    let status_text = format!(
        "Running: {} | Completed: {} | Failed: {} | Press 'h' for help, 'q' to quit",
        running_count, completed_count, failed_count
    );

    let footer = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
        );

    f.render_widget(footer, area);
}

/// Create a text-based progress bar
fn create_progress_bar(progress: f32, width: usize) -> String {
    let filled = ((progress * width as f32) as usize).min(width);
    let empty = width - filled;

    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(empty)
    )
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Run the job monitor TUI
pub async fn run_job_monitor_tui<F>(
    mut update_fn: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut() -> Result<Vec<JobListItem>, Box<dyn std::error::Error>>,
{
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = JobMonitorTui::new();
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);

    // Main loop
    loop {
        // Update jobs periodically
        if last_tick.elapsed() >= tick_rate {
            match update_fn() {
                Ok(jobs) => app.update_jobs(jobs),
                Err(e) => {
                    // Handle error - could show in UI
                    eprintln!("Error updating jobs: {}", e);
                }
            }
            app.tick();
            last_tick = Instant::now();
        }

        // Render
        terminal.draw(|f| render_job_monitor(f, &mut app))?;

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('h') | KeyCode::Char('?') => app.toggle_help(),
                        KeyCode::Char('f') => app.cycle_filter(),
                        KeyCode::Char('r') => {
                            // Force refresh
                            match update_fn() {
                                Ok(jobs) => app.update_jobs(jobs),
                                Err(e) => eprintln!("Error refreshing jobs: {}", e),
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.select_previous(),
                        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                        KeyCode::Enter => {
                            // Show job details (already handled in render)
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
