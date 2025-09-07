//! Terminal User Interface for Spacedrive CLI

use crate::{
    infrastructure::events::{Event, EventBus, EventFilter},
    library::{manager::LibraryManager, Library},
    location::manager::LocationManager,
};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs,
        Wrap,
    },
    Frame, Terminal,
};
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

/// TUI Application state
pub struct TuiApp {
    library_manager: Arc<LibraryManager>,
    event_bus: Arc<EventBus>,
    current_library: Arc<RwLock<Option<Arc<Library>>>>,
    selected_tab: usize,
    events: Vec<Event>,
    jobs: Vec<JobInfo>,
    locations: Vec<LocationInfo>,
    should_quit: bool,
    last_update: Instant,
}

#[derive(Clone)]
struct JobInfo {
    id: String,
    job_type: String,
    status: String,
    progress: f32,
    message: String,
}

#[derive(Clone)]
struct LocationInfo {
    id: String,
    name: String,
    path: String,
    status: String,
    file_count: u64,
}

impl TuiApp {
    /// Create a new TUI application
    pub fn new(
        library_manager: Arc<LibraryManager>,
        event_bus: Arc<EventBus>,
        current_library: Arc<RwLock<Option<Arc<Library>>>>,
    ) -> Self {
        Self {
            library_manager,
            event_bus,
            current_library,
            selected_tab: 0,
            events: Vec::new(),
            jobs: Vec::new(),
            locations: Vec::new(),
            should_quit: false,
            last_update: Instant::now(),
        }
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create event subscriber
        let mut event_subscriber = self.event_bus.subscribe();

        // Initial data load
        self.load_data().await?;

        // Main loop
        loop {
            // Draw UI
            terminal.draw(|f| self.draw(f))?;

            // Handle events with timeout
            if event::poll(Duration::from_millis(100))? {
                if let CEvent::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => {
                            self.should_quit = true;
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.should_quit = true;
                        }
                        KeyCode::Tab => {
                            self.selected_tab = (self.selected_tab + 1) % 4;
                        }
                        KeyCode::BackTab => {
                            self.selected_tab = if self.selected_tab == 0 {
                                3
                            } else {
                                self.selected_tab - 1
                            };
                        }
                        _ => {}
                    }
                }
            }

            // Process events from event bus
            while let Ok(event) = event_subscriber.try_recv() {
                self.handle_event(event).await;
            }

            // Refresh data periodically
            if self.last_update.elapsed() > Duration::from_secs(1) {
                self.update_jobs().await;
                self.last_update = Instant::now();
            }

            // Check if should quit
            if self.should_quit {
                break;
            }
        }

        // Cleanup
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    /// Draw the UI
    fn draw(&self, f: &mut Frame<'_>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(3),  // Header
                    Constraint::Length(3),  // Tabs
                    Constraint::Min(0),     // Content
                    Constraint::Length(3),  // Footer
                ]
                .as_ref(),
            )
            .split(f.size());

        // Draw header
        self.draw_header(f, chunks[0]);

        // Draw tabs
        self.draw_tabs(f, chunks[1]);

        // Draw content based on selected tab
        match self.selected_tab {
            0 => self.draw_overview(f, chunks[2]),
            1 => self.draw_locations(f, chunks[2]),
            2 => self.draw_jobs(f, chunks[2]),
            3 => self.draw_events(f, chunks[2]),
            _ => {}
        }

        // Draw footer
        self.draw_footer(f, chunks[3]);
    }

    /// Draw header
    fn draw_header(&self, f: &mut Frame<'_>, area: Rect) {
        let header = Paragraph::new("Spacedrive Core CLI")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
        f.render_widget(header, area);
    }

    /// Draw tabs
    fn draw_tabs(&self, f: &mut Frame<'_>, area: Rect) {
        let titles = vec!["Overview", "Locations", "Jobs", "Events"];
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL))
            .select(self.selected_tab)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(tabs, area);
    }

    /// Draw overview tab
    fn draw_overview(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(5),   // Library info
                    Constraint::Length(5),   // Statistics
                    Constraint::Percentage(50), // Recent activity
                ]
                .as_ref(),
            )
            .split(area);

        // Library info
        let library_info = if let Ok(current) = self.current_library.try_read() {
            if let Some(lib) = current.as_ref() {
                format!(
                    "Current Library: {} ({})",
                    "Active Library", // Would get actual name
                    lib.id()
                )
            } else {
                "No library currently open".to_string()
            }
        } else {
            "Loading...".to_string()
        };

        let library_widget = Paragraph::new(library_info)
            .block(Block::default().title("Library").borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        f.render_widget(library_widget, chunks[0]);

        // Statistics
        let stats = vec![
            Line::from(vec![
                Span::raw("Locations: "),
                Span::styled(
                    format!("{}", self.locations.len()),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("Active Jobs: "),
                Span::styled(
                    format!("{}", self.jobs.iter().filter(|j| j.status == "Running").count()),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::raw("Total Events: "),
                Span::styled(
                    format!("{}", self.events.len()),
                    Style::default().fg(Color::Blue),
                ),
            ]),
        ];

        let stats_widget = Paragraph::new(stats)
            .block(Block::default().title("Statistics").borders(Borders::ALL));
        f.render_widget(stats_widget, chunks[1]);

        // Recent activity
        let recent_events: Vec<ListItem> = self
            .events
            .iter()
            .rev()
            .take(10)
            .map(|e| {
                let content = format_event_short(e);
                ListItem::new(content)
            })
            .collect();

        let activity_widget = List::new(recent_events)
            .block(Block::default().title("Recent Activity").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        f.render_widget(activity_widget, chunks[2]);
    }

    /// Draw locations tab
    fn draw_locations(&self, f: &mut Frame<'_>, area: Rect) {
        let header_cells = ["ID", "Name", "Path", "Status", "Files"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells)
            .style(Style::default().add_modifier(Modifier::BOLD))
            .height(1);

        let rows = self.locations.iter().map(|loc| {
            let cells = vec![
                Cell::from(loc.id.clone()),
                Cell::from(loc.name.clone()),
                Cell::from(loc.path.clone()),
                Cell::from(loc.status.clone()).style(match loc.status.as_str() {
                    "Indexing" => Style::default().fg(Color::Yellow),
                    "Complete" => Style::default().fg(Color::Green),
                    "Error" => Style::default().fg(Color::Red),
                    _ => Style::default(),
                }),
                Cell::from(loc.file_count.to_string()),
            ];
            Row::new(cells).height(1)
        });

        let table = Table::new(rows)
            .header(header)
            .block(Block::default().title("Locations").borders(Borders::ALL))
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(20),
                Constraint::Percentage(35),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
            ]);

        f.render_widget(table, area);
    }

    /// Draw jobs tab
    fn draw_jobs(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
            .split(area);

        // Jobs table
        let header_cells = ["ID", "Type", "Status", "Progress", "Message"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells)
            .style(Style::default().add_modifier(Modifier::BOLD))
            .height(1);

        let rows = self.jobs.iter().map(|job| {
            let cells = vec![
                Cell::from(job.id.clone()),
                Cell::from(job.job_type.clone()),
                Cell::from(job.status.clone()).style(match job.status.as_str() {
                    "Running" => Style::default().fg(Color::Yellow),
                    "Completed" => Style::default().fg(Color::Green),
                    "Failed" => Style::default().fg(Color::Red),
                    _ => Style::default(),
                }),
                Cell::from(format!("{:.1}%", job.progress * 100.0)),
                Cell::from(job.message.clone()),
            ];
            Row::new(cells).height(1)
        });

        let table = Table::new(rows)
            .header(header)
            .block(Block::default().title("Jobs").borders(Borders::ALL))
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(20),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(35),
            ]);

        f.render_widget(table, chunks[0]);

        // Overall progress
        let running_jobs = self.jobs.iter().filter(|j| j.status == "Running").count();
        let avg_progress = if running_jobs > 0 {
            self.jobs
                .iter()
                .filter(|j| j.status == "Running")
                .map(|j| j.progress)
                .sum::<f32>()
                / running_jobs as f32
        } else {
            0.0
        };

        let progress = Gauge::default()
            .block(Block::default().title("Overall Progress").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent((avg_progress * 100.0) as u16);

        f.render_widget(progress, chunks[1]);
    }

    /// Draw events tab
    fn draw_events(&self, f: &mut Frame<'_>, area: Rect) {
        let events: Vec<ListItem> = self
            .events
            .iter()
            .rev()
            .map(|e| {
                let content = format_event_detailed(e);
                ListItem::new(content)
            })
            .collect();

        let events_list = List::new(events)
            .block(Block::default().title("Events").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));

        f.render_widget(events_list, area);
    }

    /// Draw footer
    fn draw_footer(&self, f: &mut Frame<'_>, area: Rect) {
        let help = Paragraph::new(
            "Tab: Switch tabs | q: Quit | ↑↓: Navigate | Enter: Select",
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(help, area);
    }

    /// Handle incoming events
    async fn handle_event(&mut self, event: Event) {
        // Add to events list
        self.events.push(event.clone());
        if self.events.len() > 1000 {
            self.events.drain(0..500);
        }

        // Update relevant data based on event type
        match &event {
            Event::LocationAdded { .. } => {
                self.load_locations().await;
            }
            Event::LocationRemoved { .. } => {
                self.load_locations().await;
            }
            Event::JobStarted { job_id, job_type } => {
                self.jobs.push(JobInfo {
                    id: job_id.clone(),
                    job_type: job_type.clone(),
                    status: "Running".to_string(),
                    progress: 0.0,
                    message: "Started".to_string(),
                });
            }
            Event::JobProgress {
                job_id,
                progress,
                message,
            } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| &j.id == job_id) {
                    job.progress = *progress as f32;
                    if let Some(msg) = message {
                        job.message = msg.clone();
                    }
                }
            }
            Event::JobCompleted { job_id, .. } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| &j.id == job_id) {
                    job.status = "Completed".to_string();
                    job.progress = 1.0;
                }
            }
            Event::JobFailed { job_id, error, .. } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| &j.id == job_id) {
                    job.status = "Failed".to_string();
                    job.message = error.clone();
                }
            }
            _ => {}
        }
    }

    /// Load initial data
    async fn load_data(&mut self) -> Result<()> {
        self.load_locations().await;
        Ok(())
    }

    /// Load locations
    async fn load_locations(&mut self) {
        if let Ok(current) = self.current_library.read().await {
            if let Some(library) = current.as_ref() {
                let location_manager = LocationManager::new(self.event_bus.clone());
                if let Ok(locations) = location_manager.list_locations(library).await {
                    self.locations = locations
                        .into_iter()
                        .map(|loc| LocationInfo {
                            id: loc.id.to_string(),
                            name: loc.name,
                            path: loc.path.to_string_lossy().to_string(),
                            status: "Ready".to_string(), // Would get actual status
                            file_count: 0, // Would get actual count
                        })
                        .collect();
                }
            }
        }
    }

    /// Update job information
    async fn update_jobs(&mut self) {
        // Would fetch actual job status from job manager
        // For now, just clean up completed jobs after a while
        self.jobs.retain(|j| {
            j.status == "Running" || j.status == "Failed"
        });
    }
}

/// Format event for short display
fn format_event_short(event: &Event) -> String {
    match event {
        Event::LibraryCreated { name, .. } => format!("Library '{}' created", name),
        Event::LibraryOpened { name, .. } => format!("Library '{}' opened", name),
        Event::LocationAdded { path, .. } => format!("Location '{}' added", path.display()),
        Event::IndexingStarted { .. } => "Indexing started".to_string(),
        Event::IndexingCompleted { total_files, .. } => {
            format!("Indexing completed: {} files", total_files)
        }
        Event::JobStarted { job_type, .. } => format!("{} started", job_type),
        Event::JobCompleted { job_type, .. } => format!("{} completed", job_type),
        _ => format!("{:?}", event).chars().take(50).collect(),
    }
}

/// Format event for detailed display
fn format_event_detailed(event: &Event) -> Text {
    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
    let header = Line::from(vec![
        Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
    ]);

    let content = match event {
        Event::LibraryCreated { id, name, path } => {
            vec![
                header,
                Line::from(vec![
                    Span::styled("Library Created: ", Style::default().fg(Color::Green)),
                    Span::raw(name),
                ]),
                Line::from(vec![
                    Span::raw("  ID: "),
                    Span::styled(id.to_string(), Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::raw("  Path: "),
                    Span::styled(
                        path.display().to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ]
        }
        Event::IndexingProgress {
            location_id,
            processed,
            total,
        } => {
            let progress = if let Some(t) = total {
                format!("{}/{}", processed, t)
            } else {
                processed.to_string()
            };
            vec![
                header,
                Line::from(vec![
                    Span::styled("Indexing Progress: ", Style::default().fg(Color::Yellow)),
                    Span::raw(progress),
                ]),
                Line::from(vec![
                    Span::raw("  Location: "),
                    Span::styled(
                        location_id.to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ]
        }
        _ => vec![header, Line::from(format!("{:?}", event))],
    };

    Text::from(content)
}