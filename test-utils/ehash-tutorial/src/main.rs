mod chapters;
mod commands;
mod config_gen;
mod process;
mod state;

use anyhow::Result;
use arboard::Clipboard;
use chapters::get_chapter_content;
use commands::CommandSystem;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ehashimint::ProcessManager;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use state::TutorialStateMachine;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

/// Result from a background command execution
#[derive(Debug, Clone)]
enum CommandResult {
    Success { message: String },
    Failed { message: String },
}

/// Status of a managed process in the tutorial
#[derive(Debug, Clone)]
pub enum ProcessStatus {
    NotStarted,
    Starting,
    Running { pid: u32 },
    Failed { error: String },
    Stopped,
}

struct App {
    should_quit: bool,
    state_machine: TutorialStateMachine,
    command_system: CommandSystem,
    input: Input,
    command_history: Vec<String>,
    history_index: Option<usize>,
    message: Option<String>,
    show_help: bool,
    // Process management
    process_manager: ProcessManager,
    process_status: HashMap<String, ProcessStatus>,
    test_dir: PathBuf,
    // Message scrolling
    message_scroll: u16,
    // Clipboard
    clipboard: Option<Clipboard>,
    copy_feedback: Option<String>,
    // Background command execution
    command_result_rx: mpsc::UnboundedReceiver<CommandResult>,
    command_result_tx: mpsc::UnboundedSender<CommandResult>,
    is_command_running: bool,
    spinner_state: usize,
}

impl App {
    fn new() -> Self {
        // Set up test directory for this tutorial session
        let test_dir = std::env::temp_dir()
            .join(format!("ehash-tutorial-{}", std::process::id()));

        let process_manager = ProcessManager::new(test_dir.clone());

        // Try to initialize clipboard (may fail in headless environments)
        let clipboard = Clipboard::new().ok();

        // Create channel for background command results
        let (command_result_tx, command_result_rx) = mpsc::unbounded_channel();

        Self {
            should_quit: false,
            state_machine: TutorialStateMachine::new(),
            command_system: CommandSystem::new(),
            input: Input::default(),
            command_history: Vec::new(),
            history_index: None,
            message: None,
            show_help: false,
            process_manager,
            process_status: HashMap::new(),
            test_dir,
            message_scroll: 0,
            clipboard,
            copy_feedback: None,
            command_result_rx,
            command_result_tx,
            is_command_running: false,
            spinner_state: 0,
        }
    }

    async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run main loop
        let res = self.main_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;

        // Return result
        res
    }

    async fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while !self.should_quit {
            // Update spinner
            if self.is_command_running {
                self.spinner_state = (self.spinner_state + 1) % 8;
            }

            terminal.draw(|f| self.ui(f))?;

            // Check for command results from background tasks
            while let Ok(result) = self.command_result_rx.try_recv() {
                self.is_command_running = false;
                match result {
                    CommandResult::Success { message } => {
                        self.message = Some(message);
                    }
                    CommandResult::Failed { message } => {
                        self.message = Some(message);
                    }
                }
            }

            self.handle_events().await?;
        }
        Ok(())
    }

    fn ui(&self, f: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Header
                Constraint::Length(5),   // Process status
                Constraint::Percentage(40), // Content (40% of screen)
                Constraint::Min(12),     // Command input + help/messages (expandable)
                Constraint::Length(2),   // Footer
            ])
            .split(f.area());

        // Header with chapter info
        let current_state = self.state_machine.current_state();
        let (current, total) = self.state_machine.progress();
        let header_text = format!(
            "Interactive eHash Tutorial - {} ({}/{})",
            current_state.display_name(),
            current,
            total
        );
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(header, chunks[0]);

        // Process status
        let status_widget = self.get_process_status_widget();
        f.render_widget(status_widget, chunks[1]);

        // Content - chapter-specific
        let content = self.get_chapter_content_widget();
        f.render_widget(content, chunks[2]);

        // Command input area
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Input field
                Constraint::Min(8),     // Help text or message (expandable)
            ])
            .split(chunks[3]);

        // Input field with placeholder text
        let width = input_chunks[0].width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);

        // Show placeholder text if input is empty
        let input_value = self.input.value();
        let display_text = if input_value.is_empty() {
            self.get_placeholder_text()
        } else {
            input_value.to_string()
        };

        let input_style = if input_value.is_empty() {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let input_widget = Paragraph::new(display_text)
            .style(input_style)
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Command (Tab for completion, ↑↓ for history)")
            );
        f.render_widget(input_widget, input_chunks[0]);

        // Show cursor in input (only if there's actual input)
        if !self.input.value().is_empty() {
            f.set_cursor_position((
                input_chunks[0].x + ((self.input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
                input_chunks[0].y + 1,
            ));
        } else {
            // Show cursor at start when input is empty
            f.set_cursor_position((
                input_chunks[0].x + 1,
                input_chunks[0].y + 1,
            ));
        }

        // Help text or message (scrollable)
        let help_content = if self.show_help {
            let commands = self.command_system.get_available_commands(current_state.display_name());
            Paragraph::new(commands.join("\n"))
                .style(Style::default().fg(Color::Green))
                .wrap(Wrap { trim: true })
                .scroll((self.message_scroll, 0))
                .block(Block::default().borders(Borders::ALL).title("Available Commands"))
        } else if self.message.is_some() {
            let msg = self.message.as_ref().unwrap();
            let line_count = msg.lines().count();
            let visible_lines = input_chunks[1].height.saturating_sub(2) as usize;

            let spinner = if self.is_command_running {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
                format!("{} ", frames[self.spinner_state % frames.len()])
            } else {
                String::new()
            };

            let title = if line_count > visible_lines {
                format!("{}Message [y: copy | ↑↓: scroll | Esc: exit] ({}/{})",
                    spinner,
                    self.message_scroll + 1,
                    line_count)
            } else if self.is_command_running {
                format!("{}Running...", spinner)
            } else {
                "Message [y: copy to clipboard | Esc: clear]".to_string()
            };

            // Build display text with optional copy feedback
            let mut lines = Vec::new();
            if let Some(ref feedback) = self.copy_feedback {
                lines.push(Line::from(Span::styled(
                    feedback.clone(),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
            }
            lines.extend(msg.lines().map(|s| Line::from(s.to_string())));

            Paragraph::new(lines)
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true })
                .scroll((self.message_scroll, 0))
                .block(Block::default().borders(Borders::ALL).title(title))
        } else {
            Paragraph::new("Type 'help' to see available commands")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL).title("Hint"))
        };
        f.render_widget(help_content, input_chunks[1]);

        // Footer - only show if there's space
        if chunks[4].height >= 3 {
            let footer_text = if self.message.is_some() {
                "MESSAGE MODE: y to copy | ↑↓ PgUp/PgDn to scroll | Esc to clear | Ctrl+C to quit"
            } else {
                "Ctrl+C: quit | →: accept suggestion | Tab: complete | ↑↓: history | Enter: execute"
            };
            let footer = Paragraph::new(footer_text)
                .style(Style::default().fg(if self.message.is_some() { Color::Yellow } else { Color::Gray }))
                .block(Block::default().borders(Borders::ALL).title("Keyboard Shortcuts"));
            f.render_widget(footer, chunks[4]);
        }
    }

    fn get_placeholder_text(&self) -> String {
        let current_state = self.state_machine.current_state();

        // Determine what command to suggest based on state and process status
        match current_state {
            state::TutorialState::Setup => {
                use ehashimint::find_binary;

                // Check if submodules are initialized by checking if deps/cdk exists
                let submodules_init = std::path::Path::new("/home/ethan/code/ehash/deps/cdk/Cargo.toml").exists()
                    || std::path::Path::new("../../deps/cdk/Cargo.toml").exists();

                if !submodules_init {
                    "git submodule update --init --recursive".to_string()
                } else {
                    // Suggest builds based on what's missing
                    let pool_built = find_binary("pool_sv2").is_ok();
                    let tproxy_built = find_binary("translator_sv2").is_ok();
                    let miner_built = find_binary("mining_device").is_ok();

                    if !pool_built || !tproxy_built {
                        "cargo build -p pool_sv2 -p translator_sv2".to_string()
                    } else if !miner_built {
                        "cargo build -p mining_device".to_string()
                    } else {
                        "next".to_string()
                    }
                }
            }
            state::TutorialState::Welcome => {
                "next".to_string()
            }
            state::TutorialState::PoolOperator => {
                // Check if pool is already running
                if let Some(ProcessStatus::Running { .. }) = self.process_status.get("pool") {
                    "next".to_string()
                } else {
                    "pool_sv2 --config pool-config-ehash.toml".to_string()
                }
            }
            state::TutorialState::ProxyOperator => {
                "cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338".to_string()
            }
            state::TutorialState::Pioneer => {
                "cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338".to_string()
            }
            state::TutorialState::Complete => {
                "back (or Ctrl+C to exit)".to_string()
            }
        }
    }

    fn get_process_status_widget(&self) -> Paragraph<'_> {
        let mut status_lines = Vec::new();

        if self.process_status.is_empty() {
            status_lines.push("No processes running".to_string());
        } else {
            for (name, status) in &self.process_status {
                let status_str = match status {
                    ProcessStatus::NotStarted => format!("{}: ○ Not started", name),
                    ProcessStatus::Starting => format!("{}: ⟳ Starting...", name),
                    ProcessStatus::Running { pid } => format!("{}: ✓ Running (PID: {})", name, pid),
                    ProcessStatus::Failed { error } => format!("{}: ✗ Failed ({})", name, error),
                    ProcessStatus::Stopped => format!("{}: ◆ Stopped", name),
                };
                status_lines.push(status_str);
            }
        }

        let text = status_lines.join("\n");
        Paragraph::new(text)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("Process Status"))
    }

    fn get_chapter_content_widget(&self) -> Paragraph<'_> {
        let current_state = self.state_machine.current_state();
        let content_lines = get_chapter_content(current_state);

        Paragraph::new(content_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Tutorial Content"))
    }

    async fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.should_quit = true;
                    }
                    KeyCode::Esc => {
                        // Clear message/help instead of quitting
                        if self.message.is_some() || self.show_help {
                            self.message = None;
                            self.show_help = false;
                            self.message_scroll = 0;
                            self.copy_feedback = None;
                        } else {
                            // If nothing to clear, quit
                            self.should_quit = true;
                        }
                    }
                    KeyCode::Char('y') => {
                        // 'y' yanks (copies) the message to clipboard
                        if self.message.is_some() {
                            self.copy_message_to_clipboard();
                        } else {
                            // Normal 'y' input
                            self.show_help = false;
                            self.history_index = None;
                            self.input.handle_event(&Event::Key(key));
                        }
                    }
                    KeyCode::Enter => {
                        self.execute_command().await;
                    }
                    KeyCode::Tab => {
                        self.handle_tab_completion();
                    }
                    KeyCode::Up => {
                        // If there's a message, scroll it. Otherwise, navigate history
                        if self.message.is_some() {
                            self.message_scroll = self.message_scroll.saturating_sub(1);
                        } else {
                            self.navigate_history_up();
                        }
                    }
                    KeyCode::Down => {
                        // If there's a message, scroll it. Otherwise, navigate history
                        if self.message.is_some() {
                            self.message_scroll = self.message_scroll.saturating_add(1);
                        } else {
                            self.navigate_history_down();
                        }
                    }
                    KeyCode::PageUp => {
                        // Scroll message faster
                        self.message_scroll = self.message_scroll.saturating_sub(5);
                    }
                    KeyCode::PageDown => {
                        // Scroll message faster
                        self.message_scroll = self.message_scroll.saturating_add(5);
                    }
                    KeyCode::Right => {
                        // If input is empty, auto-complete with placeholder text
                        if self.input.value().is_empty() {
                            let placeholder = self.get_placeholder_text();
                            self.input = Input::default().with_value(placeholder.clone());
                            // Move cursor to end
                            for _ in 0..placeholder.len() {
                                self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
                            }
                        } else {
                            // Normal right arrow behavior
                            self.show_help = false;
                            self.history_index = None;
                            self.input.handle_event(&Event::Key(key));
                        }
                    }
                    _ => {
                        // Reset help display when typing
                        self.show_help = false;
                        self.history_index = None;
                        // Handle normal input
                        self.input.handle_event(&Event::Key(key));
                    }
                }
            }
        }
        Ok(())
    }

    async fn execute_command(&mut self) {
        let command = self.input.value().trim().to_string();

        if command.is_empty() {
            return;
        }

        // Add to history
        if !self.command_history.contains(&command) {
            self.command_history.push(command.clone());
        }

        // Clear input and reset scroll
        self.input.reset();
        self.history_index = None;
        self.message_scroll = 0;
        self.copy_feedback = None;

        // Handle special commands
        if command == "help" {
            self.show_help = true;
            self.message = None;
            return;
        }

        self.show_help = false;

        // Validate command
        match self.command_system.validate_command(&command) {
            Ok(()) => {
                // Handle navigation commands
                if self.command_system.is_navigation_command(&command) {
                    let transition = self.state_machine.handle_command(&command);
                    match self.state_machine.transition(transition) {
                        Ok(()) => {
                            self.message = Some(format!("Navigated to: {}", self.state_machine.current_state().display_name()));
                        }
                        Err(e) => {
                            self.message = Some(format!("Navigation error: {}", e));
                        }
                    }
                } else {
                    // Execute actual commands
                    self.execute_real_command(&command).await;
                }
            }
            Err(e) => {
                self.message = Some(format!("Error: {}", e));
            }
        }
    }

    async fn execute_real_command(&mut self, command: &str) {
        // Handle git submodule commands
        if command.starts_with("git submodule") {
            self.run_git_submodule(command).await;
        }
        // Handle cargo build commands
        else if command.starts_with("cargo build") {
            self.run_cargo_build(command).await;
        }
        // Handle pool_sv2 command
        else if command == "pool_sv2 --config pool-config-ehash.toml" {
            self.spawn_pool().await;
        } else {
            // For other commands, show placeholder message
            self.message = Some(format!("Command recognized: {} (full execution coming soon)", command));
        }
    }

    async fn run_git_submodule(&mut self, command: &str) {
        if self.is_command_running {
            self.message = Some("⚠️  A command is already running. Please wait...".to_string());
            return;
        }

        self.is_command_running = true;
        self.message = Some(format!("⚙️  Running: {}\n\nInitializing submodules...", command));

        let command = command.to_string();
        let tx = self.command_result_tx.clone();

        // Spawn background task
        tokio::spawn(async move {
            use tokio::process::Command;

            // Parse the git command
            let parts: Vec<&str> = command.split_whitespace().collect();

            // Run git submodule in the workspace root
            let mut cmd = Command::new("git");
            for part in &parts[1..] {  // Skip "git"
                cmd.arg(part);
            }

            // Find workspace root
            let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let workspace_root = {
                let mut current = current_dir.clone();
                loop {
                    if current.join(".git").exists() {
                        break Some(current);
                    }
                    if !current.pop() {
                        break None;
                    }
                }
            };

            if let Some(root) = &workspace_root {
                cmd.current_dir(root);
            }

            // Execute the command
            let result = match cmd.output().await {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                    if output.status.success() {
                        let all_output: Vec<&str> = stderr.lines().chain(stdout.lines()).collect();
                        let last_lines: Vec<&str> = all_output.iter().rev().take(10).copied().collect();
                        let last_lines: Vec<&str> = last_lines.into_iter().rev().collect();

                        CommandResult::Success {
                            message: format!(
                                "✓ Submodules initialized!\n\n\
                                You can now build the binaries.\n\n\
                                Output:\n{}",
                                if !last_lines.is_empty() {
                                    last_lines.join("\n")
                                } else {
                                    "(Submodules already initialized)".to_string()
                                }
                            )
                        }
                    } else {
                        let error_lines: Vec<&str> = stderr.lines().chain(stdout.lines()).collect();
                        CommandResult::Failed {
                            message: format!(
                                "✗ Submodule init failed!\n\n\
                                Working directory: {}\n\
                                Command: git {}\n\n\
                                Error:\n{}",
                                workspace_root.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Unknown".to_string()),
                                parts[1..].join(" "),
                                error_lines.join("\n")
                            )
                        }
                    }
                }
                Err(e) => {
                    CommandResult::Failed {
                        message: format!(
                            "✗ Failed to run git: {}\n\n\
                            Make sure git is installed and you're in a git repository",
                            e
                        )
                    }
                }
            };

            let _ = tx.send(result);
        });
    }

    fn copy_message_to_clipboard(&mut self) {
        if let Some(ref msg) = self.message {
            match &mut self.clipboard {
                Some(clipboard) => {
                    match clipboard.set_text(msg.clone()) {
                        Ok(()) => {
                            self.copy_feedback = Some(format!(
                                "✓ Copied {} bytes to clipboard!",
                                msg.len()
                            ));
                        }
                        Err(e) => {
                            self.copy_feedback = Some(format!(
                                "✗ Failed to copy to clipboard: {}",
                                e
                            ));
                        }
                    }
                }
                None => {
                    self.copy_feedback = Some(
                        "✗ Clipboard not available (headless environment?)".to_string()
                    );
                }
            }
        }
    }

    async fn run_cargo_build(&mut self, command: &str) {
        if self.is_command_running {
            self.message = Some("⚠️  A command is already running. Please wait...".to_string());
            return;
        }

        self.is_command_running = true;
        self.message = Some(format!("⚙️  Running: {}\n\nThis will take a few minutes...", command));

        let command = command.to_string();
        let tx = self.command_result_tx.clone();

        // Spawn background task
        tokio::spawn(async move {
            use tokio::process::Command;

            // Parse the cargo command
            let parts: Vec<&str> = command.split_whitespace().collect();

            // Run cargo build
            let mut cmd = Command::new("cargo");
            for part in &parts[1..] {  // Skip "cargo"
                cmd.arg(part);
            }

            // Set working directory to the workspace root (try to find it)
            let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

            let workspace_root = {
                let mut current = current_dir.clone();
                loop {
                    if current.join("roles").join("Cargo.toml").exists() {
                        break Some(current);
                    }
                    if !current.pop() {
                        break None;
                    }
                }
            };

            let build_dir = match &workspace_root {
                Some(root) => {
                    let roles_dir = root.join("roles");
                    cmd.current_dir(&roles_dir);
                    roles_dir
                }
                None => {
                    // Try current directory
                    cmd.current_dir(&current_dir);
                    current_dir.clone()
                }
            };

            // Execute the command
            let result = match cmd.output().await {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let exit_code = output.status.code().unwrap_or(-1);

                    if output.status.success() {
                        // Show last few lines of output (cargo writes to stderr even on success)
                        let all_output: Vec<&str> = stderr.lines().chain(stdout.lines()).collect();
                        let last_lines: Vec<&str> = all_output.iter().rev().take(8).copied().collect();
                        let last_lines: Vec<&str> = last_lines.into_iter().rev().collect();

                        CommandResult::Success {
                            message: format!(
                                "✓ Build succeeded!\n\n\
                                The binaries have been built. Check the status above to verify.\n\n\
                                Last output:\n{}",
                                if !last_lines.is_empty() {
                                    last_lines.join("\n")
                                } else {
                                    "(No output captured)".to_string()
                                }
                            )
                        }
                    } else {
                        // Show error with context
                        let all_output: Vec<&str> = stderr.lines().chain(stdout.lines()).collect();
                        let error_lines: Vec<&str> = all_output.iter().rev().take(25).copied().collect();
                        let error_lines: Vec<&str> = error_lines.into_iter().rev().collect();

                        let error_text = if !error_lines.is_empty() {
                            error_lines.join("\n")
                        } else if !stderr.is_empty() {
                            stderr.clone()
                        } else if !stdout.is_empty() {
                            stdout.clone()
                        } else {
                            format!(
                                "No output captured.\n\n\
                                Exit code: {}\n\
                                Stdout length: {} bytes\n\
                                Stderr length: {} bytes\n\n\
                                This might mean:\n\
                                • The package doesn't exist in this workspace\n\
                                • Cargo.toml is missing or invalid\n\
                                • Try running the command manually to see full output",
                                exit_code,
                                stdout.len(),
                                stderr.len()
                            )
                        };

                        CommandResult::Failed {
                            message: format!(
                                "✗ Build failed! (exit code: {})\n\n\
                                Working directory: {}\n\
                                Command: cargo {}\n\n\
                                Error output:\n{}",
                                exit_code,
                                build_dir.display(),
                                parts[1..].join(" "),
                                error_text
                            )
                        }
                    }
                }
                Err(e) => {
                    CommandResult::Failed {
                        message: format!(
                            "✗ Failed to run cargo: {}\n\n\
                            Current directory: {}\n\
                            Detected workspace: {:?}\n\n\
                            Make sure:\n\
                            • cargo is installed\n\
                            • You're in the ehash repository\n\
                            • The roles/ directory exists",
                            e,
                            current_dir.display(),
                            workspace_root.as_ref().map(|p| p.display().to_string())
                        )
                    }
                }
            };

            let _ = tx.send(result);
        });
    }

    async fn spawn_pool(&mut self) {
        use crate::config_gen::generate_pool_config;
        use ehashimint::find_binary;

        // Generate config if needed
        let config_path = self.test_dir.join("configs").join("pool-config-ehash.toml");

        // Create directories
        if let Err(e) = tokio::fs::create_dir_all(config_path.parent().unwrap()).await {
            self.message = Some(format!("Failed to create config directory: {}", e));
            return;
        }

        if let Err(e) = generate_pool_config(&config_path).await {
            self.message = Some(format!("Failed to generate pool config: {}", e));
            return;
        }

        // Find pool binary
        let pool_binary = match find_binary("pool_sv2") {
            Ok(bin) => bin,
            Err(e) => {
                self.message = Some(format!(
                    "❌ pool_sv2 binary not found!\n\n\
                    Please build the Stratum v2 Pool binary first:\n\
                    \n\
                    For quick testing (debug mode):\n\
                    → cd /path/to/stratum/roles && cargo build -p pool_sv2\n\
                    \n\
                    For production (optimized):\n\
                    → cd /path/to/stratum/roles && cargo build --release -p pool_sv2\n\
                    \n\
                    Error details: {}", e
                ));
                return;
            }
        };

        // Spawn the process
        match self.process_manager.spawn("pool", &pool_binary, &[], &config_path).await {
            Ok(()) => {
                // Get the PID from the process manager
                if let Some(process) = self.process_manager.processes.last() {
                    if let Some(pid) = process.child.id() {
                        self.process_status.insert("pool".to_string(), ProcessStatus::Running { pid });
                        self.message = Some(format!(
                            "✓ Pool started successfully!\n\n\
                            PID: {}\n\
                            Config: {}\n\
                            Logs: {}\n\n\
                            Verify it's running:\n\
                            → ps aux | grep -E '(pool_sv2|translator_sv2|mining_device)'\n\n\
                            Or continue to the next chapter:\n\
                            → next",
                            pid,
                            config_path.display(),
                            self.test_dir.join("logs").join("pool.log").display()
                        ));
                    } else {
                        self.message = Some("Pool started but couldn't get PID".to_string());
                    }
                } else {
                    self.message = Some("Pool started but couldn't verify".to_string());
                }
            }
            Err(e) => {
                self.process_status.insert("pool".to_string(), ProcessStatus::Failed {
                    error: e.to_string()
                });
                self.message = Some(format!("Failed to start pool: {}", e));
            }
        }
    }

    fn handle_tab_completion(&mut self) {
        let partial = self.input.value();
        let completions = self.command_system.get_completions(partial);

        if completions.is_empty() {
            self.message = Some("No completions available".to_string());
        } else if completions.len() == 1 {
            // Single completion - apply it
            self.input = Input::default().with_value(completions[0].clone());
            // Move cursor to end
            for _ in 0..completions[0].len() {
                self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
            }
            self.message = None;
        } else {
            // Multiple completions - show them
            self.message = Some(format!("Completions: {}", completions.join(", ")));
        }
    }

    fn navigate_history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => self.command_history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };

        self.history_index = Some(new_index);
        self.input = Input::default().with_value(self.command_history[new_index].clone());

        // Move cursor to end
        for _ in 0..self.command_history[new_index].len() {
            self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
        }
    }

    fn navigate_history_down(&mut self) {
        if self.command_history.is_empty() || self.history_index.is_none() {
            return;
        }

        let current_index = self.history_index.unwrap();
        if current_index >= self.command_history.len() - 1 {
            // At the end - clear input
            self.history_index = None;
            self.input.reset();
        } else {
            let new_index = current_index + 1;
            self.history_index = Some(new_index);
            self.input = Input::default().with_value(self.command_history[new_index].clone());

            // Move cursor to end
            for _ in 0..self.command_history[new_index].len() {
                self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();
    app.run().await?;
    Ok(())
}
