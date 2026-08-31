use std::{
    collections::VecDeque,
    io::{self, Write},
    time::{Duration, Instant},
};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use mcg_config::CliTuiPreferences;
use mcg_domain::{
    ComposerMentionInput, ConversationActivityKind, ConversationActivityState,
    ConversationAgentSessionId, ConversationAgentSessionState, ConversationInputQuestion,
    ConversationRole, ConversationTurnState, ConversationView, MentionCandidate,
};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

const MAX_VISIBLE_CHOICES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPickerCandidate {
    pub id: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteKind {
    Slash,
    Mention,
    Dollar,
    CommandOption,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCandidate {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteCandidate {
    Slash(SlashCandidate),
    Mention(MentionCandidate),
    Reference {
        value: String,
        label: String,
        detail: String,
    },
    Choice {
        value: String,
        label: String,
        detail: String,
    },
}

impl PaletteCandidate {
    fn label(&self) -> String {
        match self {
            Self::Slash(candidate) => format!("/{}", candidate.name),
            Self::Mention(candidate) => format!("@{}", candidate.label),
            Self::Reference { label, .. } => format!("${label}"),
            Self::Choice { label, .. } => label.clone(),
        }
    }

    fn detail(&self) -> &str {
        match self {
            Self::Slash(candidate) => candidate.description,
            Self::Mention(candidate) => candidate.detail.as_deref().unwrap_or(&candidate.resource),
            Self::Reference { detail, .. } => detail,
            Self::Choice { detail, .. } => detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteRequest {
    pub kind: PaletteKind,
    pub query: String,
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedMention {
    start: usize,
    end: usize,
    candidate: MentionCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedLine {
    pub content: String,
    pub mentions: Vec<ComposerMentionInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction {
    Submit(SubmittedLine),
    Steer(SubmittedLine),
    Queue(SubmittedLine),
    MessageAgent {
        session_id: ConversationAgentSessionId,
        content: String,
    },
    CopyLatest,
    ForkPrevious,
    AnswerInput {
        request_id: serde_json::Value,
        answers: serde_json::Value,
    },
    Exit,
}

#[derive(Debug, Clone)]
struct QuestionOverlay {
    request_id: serde_json::Value,
    questions: Vec<ConversationInputQuestion>,
    question_index: usize,
    selected: usize,
    text: String,
    other_mode: bool,
    answers: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
struct AgentPickerOverlay {
    query: String,
    selected: usize,
}

impl QuestionOverlay {
    fn new(request: &mcg_domain::ConversationInputRequest) -> Self {
        Self {
            request_id: request.request_id.clone(),
            questions: request.questions.clone(),
            question_index: 0,
            selected: 0,
            text: String::new(),
            other_mode: false,
            answers: serde_json::Map::new(),
        }
    }

    fn question(&self) -> &ConversationInputQuestion {
        &self.questions[self.question_index]
    }
}

pub struct TuiTerminal {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    alternate_screen: bool,
}

impl TuiTerminal {
    pub fn enter(no_alt_screen: bool) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if no_alt_screen {
            execute!(stdout, event::EnableMouseCapture, cursor::Hide)?;
        } else {
            execute!(
                stdout,
                EnterAlternateScreen,
                event::EnableMouseCapture,
                cursor::Hide
            )?;
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = if no_alt_screen {
            let (_, rows) = terminal::size()?;
            Terminal::with_options(
                backend,
                TerminalOptions {
                    viewport: Viewport::Inline(rows.saturating_sub(1).max(1)),
                },
            )?
        } else {
            Terminal::new(backend)?
        };
        Ok(Self {
            terminal,
            alternate_screen: !no_alt_screen,
        })
    }

    pub fn draw(&mut self, model: &mut TuiModel) -> io::Result<()> {
        execute!(
            self.terminal.backend_mut(),
            terminal::SetTitle(model.terminal_title())
        )?;
        self.terminal.draw(|frame| model.render(frame))?;
        Ok(())
    }
}

impl Drop for TuiTerminal {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        if self.alternate_screen {
            let _ = execute!(
                self.terminal.backend_mut(),
                event::DisableMouseCapture,
                LeaveAlternateScreen,
                cursor::Show
            );
        } else {
            let _ = execute!(
                self.terminal.backend_mut(),
                event::DisableMouseCapture,
                cursor::Show
            );
        }
        let _ = self.terminal.show_cursor();
    }
}

struct SessionPickerTerminal {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    alternate_screen: bool,
}

impl SessionPickerTerminal {
    fn enter(no_alt_screen: bool) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if no_alt_screen {
            execute!(stdout, event::EnableMouseCapture, cursor::Hide)?;
        } else {
            execute!(
                stdout,
                EnterAlternateScreen,
                event::EnableMouseCapture,
                cursor::Hide
            )?;
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = if no_alt_screen {
            let (_, rows) = terminal::size()?;
            Terminal::with_options(
                backend,
                TerminalOptions {
                    viewport: Viewport::Inline(rows.saturating_sub(1).clamp(1, 18)),
                },
            )?
        } else {
            Terminal::new(backend)?
        };
        Ok(Self {
            terminal,
            alternate_screen: !no_alt_screen,
        })
    }
}

impl Drop for SessionPickerTerminal {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        if self.alternate_screen {
            let _ = execute!(
                self.terminal.backend_mut(),
                event::DisableMouseCapture,
                LeaveAlternateScreen,
                cursor::Show
            );
        } else {
            let _ = execute!(
                self.terminal.backend_mut(),
                event::DisableMouseCapture,
                cursor::Show
            );
        }
        let _ = self.terminal.show_cursor();
    }
}

fn fuzzy_session_indices(candidates: &[SessionPickerCandidate], query: &str) -> Vec<usize> {
    let needle = query.trim().to_ascii_lowercase();
    let mut matches = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            if needle.is_empty() {
                return Some((index, 0_usize));
            }
            let haystack = format!("{} {} {}", candidate.title, candidate.detail, candidate.id)
                .to_ascii_lowercase();
            if let Some(position) = haystack.find(&needle) {
                return Some((index, position));
            }
            let mut cursor = 0_usize;
            for character in needle.chars() {
                let relative = haystack[cursor..].find(character)?;
                cursor += relative + character.len_utf8();
            }
            Some((index, cursor + haystack.len()))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(index, score)| (*score, *index));
    matches.into_iter().map(|(index, _)| index).collect()
}

fn fuzzy_subsequence(haystack: &str, needle: &str) -> bool {
    let mut cursor = 0_usize;
    for character in needle.chars() {
        let Some(relative) = haystack[cursor..].find(character) else {
            return false;
        };
        cursor += relative + character.len_utf8();
    }
    true
}

fn render_session_picker(
    frame: &mut ratatui::Frame<'_>,
    candidates: &[SessionPickerCandidate],
    filtered: &[usize],
    query: &str,
    selected: usize,
    heading: &str,
    search_title: &str,
    empty_footer: &str,
    active_footer: &str,
) {
    let area = frame.area();
    let content = if area.width > 96 {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(18),
                Constraint::Percentage(64),
                Constraint::Percentage(18),
            ])
            .split(area);
        horizontal[1]
    } else {
        area
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(content);
    frame.render_widget(
        Paragraph::new(heading).style(Style::default().add_modifier(Modifier::BOLD)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(query).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {search_title} ")),
        ),
        rows[1],
    );
    let visible_height = rows[2].height.saturating_sub(2) as usize;
    let offset = selected.saturating_sub(visible_height.saturating_sub(1));
    let items = filtered
        .iter()
        .skip(offset)
        .take(visible_height.max(1))
        .map(|index| {
            let candidate = &candidates[*index];
            ListItem::new(vec![
                Line::from(Span::styled(
                    candidate.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    candidate.detail.clone(),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default()
        .with_selected((!items.is_empty()).then_some(selected.saturating_sub(offset)));
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray)),
        rows[2],
        &mut state,
    );
    let footer = if filtered.is_empty() {
        empty_footer
    } else {
        active_footer
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
        rows[3],
    );
}

fn select_candidate(
    candidates: &[SessionPickerCandidate],
    no_alt_screen: bool,
    heading: &str,
    search_title: &str,
    empty_footer: &str,
    active_footer: &str,
) -> io::Result<Option<String>> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let mut terminal = SessionPickerTerminal::enter(no_alt_screen)?;
    let mut query = String::new();
    let mut selected = 0_usize;
    loop {
        let filtered = fuzzy_session_indices(candidates, &query);
        selected = selected.min(filtered.len().saturating_sub(1));
        terminal.terminal.draw(|frame| {
            render_session_picker(
                frame,
                candidates,
                &filtered,
                &query,
                selected,
                heading,
                search_title,
                empty_footer,
                active_footer,
            );
        })?;

        match event::read()? {
            Event::Key(key) if key.kind == event::KeyEventKind::Press => match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => {
                    if let Some(index) = filtered.get(selected) {
                        return Ok(Some(candidates[*index].id.clone()));
                    }
                }
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => {
                    selected = (selected + 1).min(filtered.len().saturating_sub(1));
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(character)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    query.push(character);
                    selected = 0;
                }
                _ => {}
            },
            Event::Paste(value) => {
                query.push_str(&value.replace(['\r', '\n'], ""));
                selected = 0;
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => selected = selected.saturating_sub(1),
                MouseEventKind::ScrollDown => {
                    selected = (selected + 1).min(filtered.len().saturating_sub(1));
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Opens a searchable Codex-style terminal picker and returns the selected stable id.
pub fn select_session(
    candidates: &[SessionPickerCandidate],
    no_alt_screen: bool,
) -> io::Result<Option<String>> {
    select_candidate(
        candidates,
        no_alt_screen,
        "Resume a conversation",
        "Search titles, ids, and projects",
        "No matching conversations · Esc cancels",
        "↑↓ select · Enter resume · type to filter · Esc cancel",
    )
}

/// Opens the global agent-session picker and returns the selected stable session id.
///
/// # Errors
///
/// Returns an I/O error when terminal setup, rendering, or input handling fails.
pub fn select_agent_session(
    candidates: &[SessionPickerCandidate],
    no_alt_screen: bool,
) -> io::Result<Option<String>> {
    select_candidate(
        candidates,
        no_alt_screen,
        "Browse governed agent sessions",
        "Search conversations, roles, states, or ids",
        "No matching governed agent session · Esc cancels",
        "↑↓ select · Enter open · type to filter · Esc cancel",
    )
}

/// Asks which working directory should govern a resumed or forked conversation.
pub fn select_resume_cwd(
    current: &str,
    session: &str,
    no_alt_screen: bool,
) -> io::Result<Option<String>> {
    let candidates = [
        SessionPickerCandidate {
            id: "current".into(),
            title: "Use current directory".into(),
            detail: current.into(),
        },
        SessionPickerCandidate {
            id: "session".into(),
            title: "Use session directory".into(),
            detail: session.into(),
        },
    ];
    select_candidate(
        &candidates,
        no_alt_screen,
        "Choose a working directory",
        "Filter directory choices",
        "No matching directory choice · Esc cancels",
        "↑↓ select · Enter choose · Esc cancel",
    )
}

/// Opens the same resize-safe searchable picker Codex uses when `--oss` has
/// no unambiguous configured or running provider.
pub fn select_local_provider(
    candidates: &[SessionPickerCandidate],
    no_alt_screen: bool,
) -> io::Result<Option<String>> {
    select_candidate(
        candidates,
        no_alt_screen,
        "Choose a local model provider",
        "Search Ollama or LM Studio",
        "No matching local provider · Esc cancels",
        "↑↓ select · Enter choose · Esc cancel",
    )
}

pub struct TuiModel {
    editor: EditorState,
    history: Vec<String>,
    history_index: usize,
    history_search: Option<String>,
    candidates: Vec<PaletteCandidate>,
    view: ConversationView,
    project: String,
    branch: String,
    transcript_scroll: u16,
    transcript_cleared_before: Option<chrono::DateTime<chrono::Utc>>,
    queued: VecDeque<SubmittedLine>,
    status: String,
    details_expanded: bool,
    last_escape: Option<Instant>,
    question_overlay: Option<QuestionOverlay>,
    agent_picker: Option<AgentPickerOverlay>,
    selected_agent_session_id: Option<ConversationAgentSessionId>,
    notices: VecDeque<(chrono::DateTime<chrono::Utc>, String)>,
    cli_preferences: CliTuiPreferences,
    vim_insert: bool,
}

impl TuiModel {
    #[must_use]
    pub fn new(view: ConversationView, project: String, branch: String) -> Self {
        Self {
            editor: EditorState::default(),
            history: Vec::new(),
            history_index: 0,
            history_search: None,
            candidates: Vec::new(),
            view,
            project,
            branch,
            transcript_scroll: 0,
            transcript_cleared_before: None,
            queued: VecDeque::new(),
            status: "Ready".into(),
            details_expanded: false,
            last_escape: None,
            question_overlay: None,
            agent_picker: None,
            selected_agent_session_id: None,
            notices: VecDeque::new(),
            cli_preferences: CliTuiPreferences::default(),
            vim_insert: true,
        }
    }

    pub fn set_cli_preferences(&mut self, preferences: CliTuiPreferences) {
        if !preferences.vim_mode {
            self.vim_insert = true;
        }
        self.cli_preferences = preferences;
    }

    fn terminal_title(&self) -> String {
        let mut fields = Vec::new();
        for field in &self.cli_preferences.title_fields {
            match field.as_str() {
                "project" => fields.push(self.project.clone()),
                "conversation" => fields.push(self.view.conversation.title.clone()),
                "branch" => fields.push(self.branch.clone()),
                _ => {}
            }
        }
        if fields.is_empty() {
            "MetaCog".into()
        } else {
            format!("{} · MetaCog", fields.join(" · "))
        }
    }

    pub fn set_view(&mut self, view: ConversationView) {
        self.view = view;
        if self.selected_agent_session_id.is_some_and(|selected| {
            !self
                .view
                .team
                .sessions
                .iter()
                .any(|session| session.id == selected)
        }) {
            self.selected_agent_session_id = None;
        }
        if self.question_overlay.is_none() {
            self.question_overlay = self
                .view
                .conversation
                .pending_input_requests
                .first()
                .map(QuestionOverlay::new);
        }
        self.status = if self.is_active() {
            "Working".into()
        } else {
            "Ready".into()
        };
    }

    pub fn set_status(&mut self, value: impl Into<String>) {
        self.status = value.into();
    }

    /// Focuses an exact agent session already present in the scoped conversation view.
    pub fn focus_agent_session(&mut self, session_id: ConversationAgentSessionId) -> bool {
        if self
            .view
            .team
            .sessions
            .iter()
            .any(|session| session.id == session_id)
        {
            self.selected_agent_session_id = Some(session_id);
            self.transcript_scroll = 0;
            self.status = format!("Viewing {}", self.selected_agent_name());
            true
        } else {
            false
        }
    }

    pub fn push_notices(&mut self, notices: Vec<String>) {
        self.notices.extend(
            notices
                .into_iter()
                .map(|notice| (chrono::Utc::now(), notice)),
        );
        while self.notices.len() > 20 {
            self.notices.pop_front();
        }
    }

    pub fn set_workspace(&mut self, project: String, branch: String) {
        self.project = project;
        self.branch = branch;
    }

    pub fn set_candidates(&mut self, candidates: Vec<PaletteCandidate>) {
        self.candidates = candidates;
        self.editor.clamp_selection(self.candidates.len());
    }

    #[must_use]
    pub fn palette_request(&self) -> Option<PaletteRequest> {
        self.editor.palette_request()
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.view.turns.iter().any(|turn| {
            matches!(
                turn.state,
                ConversationTurnState::Pending
                    | ConversationTurnState::Streaming
                    | ConversationTurnState::WaitingForApproval
                    | ConversationTurnState::Recovering
                    | ConversationTurnState::Indeterminate
            )
        })
    }

    #[must_use]
    pub fn active_turn(&self) -> Option<mcg_domain::ConversationTurn> {
        self.view
            .turns
            .iter()
            .rev()
            .find(|turn| {
                matches!(
                    turn.state,
                    ConversationTurnState::Pending
                        | ConversationTurnState::Streaming
                        | ConversationTurnState::WaitingForApproval
                        | ConversationTurnState::Recovering
                        | ConversationTurnState::Indeterminate
                )
            })
            .cloned()
    }

    #[must_use]
    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn queue(&mut self, line: SubmittedLine) {
        self.queued.push_back(line);
    }

    pub fn pop_queue(&mut self) -> Option<SubmittedLine> {
        self.queued.pop_front()
    }

    #[must_use]
    pub fn latest_assistant_text(&self) -> Option<&str> {
        if let Some(session_id) = self.selected_agent_session_id {
            return self
                .view
                .team
                .messages
                .iter()
                .rev()
                .find(|message| message.from_session_id == session_id)
                .map(|message| message.content.as_str());
        }
        self.view
            .messages
            .iter()
            .rev()
            .find(|message| message.role == ConversationRole::Assistant)
            .map(|message| message.content.as_str())
    }

    pub fn handle_event(
        &mut self,
        input: Event,
    ) -> Result<Option<TuiAction>, Box<dyn std::error::Error>> {
        match input {
            Event::Key(key) if key.kind != event::KeyEventKind::Release => self.handle_key(key),
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        self.transcript_scroll = self.transcript_scroll.saturating_add(3)
                    }
                    MouseEventKind::ScrollDown => {
                        self.transcript_scroll = self.transcript_scroll.saturating_sub(3)
                    }
                    _ => {}
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<TuiAction>, Box<dyn std::error::Error>> {
        if self.agent_picker.is_some() {
            return self.handle_agent_picker_key(key);
        }
        if self.question_overlay.is_some() {
            return self.handle_question_key(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('d') if self.editor.buffer.is_empty() => {
                    return Ok(Some(TuiAction::Exit));
                }
                KeyCode::Char('o') => return Ok(Some(TuiAction::CopyLatest)),
                KeyCode::Char('l') => {
                    self.transcript_cleared_before = Some(chrono::Utc::now());
                    self.transcript_scroll = 0;
                    self.status = "Display cleared; conversation history is retained".into();
                    return Ok(None);
                }
                KeyCode::Char('r') => {
                    self.history_search = Some(String::new());
                    return Ok(None);
                }
                KeyCode::Char('e') => {
                    self.details_expanded = !self.details_expanded;
                    return Ok(None);
                }
                _ => {}
            }
        }
        if let Some(search) = self.history_search.as_mut() {
            match key.code {
                KeyCode::Esc => self.history_search = None,
                KeyCode::Enter => {
                    if let Some(found) = self
                        .history
                        .iter()
                        .rev()
                        .find(|value| value.contains(search.as_str()))
                    {
                        self.editor.replace_all(found.clone());
                    }
                    self.history_search = None;
                }
                KeyCode::Backspace => {
                    search.pop();
                }
                KeyCode::Char(value) => search.push(value),
                _ => {}
            }
            return Ok(None);
        }
        if self.cli_preferences.vim_mode && !self.vim_insert {
            match key.code {
                KeyCode::Char('i') => self.vim_insert = true,
                KeyCode::Char('a') => {
                    self.editor.cursor = (self.editor.cursor + 1).min(self.editor.chars().len());
                    self.vim_insert = true;
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    self.editor.cursor = self.editor.cursor.saturating_sub(1)
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.editor.cursor = (self.editor.cursor + 1).min(self.editor.chars().len())
                }
                KeyCode::Char('0') | KeyCode::Home => self.editor.cursor = 0,
                KeyCode::Char('$') | KeyCode::End => self.editor.cursor = self.editor.chars().len(),
                KeyCode::Char('x') | KeyCode::Delete
                    if self.editor.cursor < self.editor.chars().len() =>
                {
                    self.editor.delete_forward();
                }
                KeyCode::Enter => return self.submit_action(false),
                _ => {}
            }
            return Ok(None);
        }
        let palette_open = self.editor.palette_request().is_some() && !self.candidates.is_empty();
        let opens_agent_picker = matches!(self.editor.buffer.trim(), "/agent" | "/subagents");
        match key.code {
            KeyCode::Enter if opens_agent_picker => return self.submit_action(false),
            KeyCode::Enter | KeyCode::Tab if palette_open => {
                self.editor.select(&self.candidates[self.editor.selected]);
            }
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
            {
                self.editor
                    .edit(self.editor.cursor, self.editor.cursor, "\n");
            }
            KeyCode::Enter => return self.submit_action(false),
            KeyCode::Tab if self.is_active() => return self.submit_action(true),
            KeyCode::Esc => {
                if self.editor.palette_request().is_some() {
                    self.editor.dismiss_palette();
                } else if self.selected_agent_session_id.is_some() && self.editor.buffer.is_empty()
                {
                    self.selected_agent_session_id = None;
                    self.status = "Returned to main thread".into();
                } else if self.cli_preferences.vim_mode && self.vim_insert {
                    self.vim_insert = false;
                } else if self.editor.buffer.is_empty()
                    && self
                        .last_escape
                        .is_some_and(|last| last.elapsed() <= Duration::from_millis(650))
                {
                    self.last_escape = None;
                    return Ok(Some(TuiAction::ForkPrevious));
                } else {
                    self.last_escape = Some(Instant::now());
                }
            }
            KeyCode::PageUp => self.transcript_scroll = self.transcript_scroll.saturating_add(8),
            KeyCode::PageDown => self.transcript_scroll = self.transcript_scroll.saturating_sub(8),
            KeyCode::Up if palette_open => {
                self.editor.selected = self.editor.selected.saturating_sub(1)
            }
            KeyCode::Down if palette_open => {
                self.editor.selected = (self.editor.selected + 1).min(self.candidates.len() - 1);
            }
            KeyCode::Up if !self.editor.buffer.contains('\n') => {
                if self.history_index > 0 {
                    self.history_index -= 1;
                    self.editor
                        .replace_all(self.history[self.history_index].clone());
                }
            }
            KeyCode::Down if !self.editor.buffer.contains('\n') => {
                if self.history_index + 1 < self.history.len() {
                    self.history_index += 1;
                    self.editor
                        .replace_all(self.history[self.history_index].clone());
                } else if self.history_index < self.history.len() {
                    self.history_index = self.history.len();
                    self.editor.replace_all(String::new());
                }
            }
            KeyCode::Left => self.editor.cursor = self.editor.cursor.saturating_sub(1),
            KeyCode::Right => {
                self.editor.cursor = (self.editor.cursor + 1).min(self.editor.chars().len())
            }
            KeyCode::Home => self.editor.cursor = 0,
            KeyCode::End => self.editor.cursor = self.editor.chars().len(),
            KeyCode::Backspace if self.editor.cursor > 0 => self.editor.delete_backward(),
            KeyCode::Delete if self.editor.cursor < self.editor.chars().len() => {
                self.editor.delete_forward()
            }
            KeyCode::Char(value)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.editor
                    .edit(self.editor.cursor, self.editor.cursor, &value.to_string());
            }
            _ => {}
        }
        Ok(None)
    }

    fn handle_question_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<TuiAction>, Box<dyn std::error::Error>> {
        let mut completed_answer = None;
        {
            let overlay = self.question_overlay.as_mut().expect("question overlay");
            let question = overlay.question().clone();
            let options = question.options.as_deref().unwrap_or_default();
            let choice_count = options.len() + usize::from(question.is_other);
            let text_mode = options.is_empty() || overlay.other_mode;
            if text_mode {
                match key.code {
                    KeyCode::Backspace => {
                        overlay.text.pop();
                    }
                    KeyCode::Char(value)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        overlay.text.push(value);
                    }
                    KeyCode::Esc if overlay.other_mode => {
                        overlay.other_mode = false;
                        overlay.text.clear();
                    }
                    KeyCode::Enter if !overlay.text.trim().is_empty() => {
                        completed_answer = Some(overlay.text.trim().to_owned());
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Up => overlay.selected = overlay.selected.saturating_sub(1),
                    KeyCode::Down => {
                        overlay.selected =
                            (overlay.selected + 1).min(choice_count.saturating_sub(1));
                    }
                    KeyCode::Char(value) if value.is_ascii_digit() => {
                        if let Some(selected) = value.to_digit(10).map(|value| value as usize) {
                            if selected > 0 && selected <= choice_count {
                                overlay.selected = selected - 1;
                            }
                        }
                    }
                    KeyCode::Enter if overlay.selected < options.len() => {
                        completed_answer = Some(options[overlay.selected].label.clone());
                    }
                    KeyCode::Enter if question.is_other => {
                        overlay.other_mode = true;
                        overlay.text.clear();
                    }
                    _ => {}
                }
            }
        }
        if let Some(answer) = completed_answer {
            return self.finish_question(answer);
        }
        Ok(None)
    }

    fn finish_question(
        &mut self,
        answer: String,
    ) -> Result<Option<TuiAction>, Box<dyn std::error::Error>> {
        let overlay = self.question_overlay.as_mut().expect("question overlay");
        let question_id = overlay.question().id.clone();
        overlay
            .answers
            .insert(question_id, serde_json::json!({ "answers": [answer] }));
        if overlay.question_index + 1 < overlay.questions.len() {
            overlay.question_index += 1;
            overlay.selected = 0;
            overlay.text.clear();
            overlay.other_mode = false;
            return Ok(None);
        }
        let overlay = self.question_overlay.take().expect("question overlay");
        Ok(Some(TuiAction::AnswerInput {
            request_id: overlay.request_id,
            answers: serde_json::Value::Object(overlay.answers),
        }))
    }

    fn submit_action(
        &mut self,
        queue: bool,
    ) -> Result<Option<TuiAction>, Box<dyn std::error::Error>> {
        let state = std::mem::take(&mut self.editor);
        let line = state.submit()?;
        if line.content.trim().is_empty() {
            return Ok(None);
        }
        if matches!(line.content.trim(), "/exit" | "/quit" | ":exit" | ":quit") {
            return Ok(Some(TuiAction::Exit));
        }
        if self.handle_agent_command(line.content.trim()) {
            return Ok(None);
        }
        self.history.push(line.content.clone());
        self.history_index = self.history.len();
        self.candidates.clear();
        if let Some(session_id) = self.selected_agent_session_id
            && !line.content.trim_start().starts_with('/')
            && !line.content.trim_start().starts_with('!')
        {
            Ok(Some(TuiAction::MessageAgent {
                session_id,
                content: line.content,
            }))
        } else if queue {
            Ok(Some(TuiAction::Queue(line)))
        } else if self.is_active() {
            Ok(Some(TuiAction::Steer(line)))
        } else {
            Ok(Some(TuiAction::Submit(line)))
        }
    }

    fn handle_agent_command(&mut self, content: &str) -> bool {
        let Some(argument) = content
            .strip_prefix("/agent")
            .or_else(|| content.strip_prefix("/subagents"))
        else {
            return false;
        };
        if !argument.is_empty() && !argument.starts_with(char::is_whitespace) {
            return false;
        }
        let selector = argument.trim();
        if selector.is_empty() {
            self.agent_picker = Some(AgentPickerOverlay {
                query: String::new(),
                selected: 0,
            });
            self.status = "Choose an agent thread".into();
            return true;
        }
        if selector.eq_ignore_ascii_case("main") || selector.eq_ignore_ascii_case("chat") {
            self.selected_agent_session_id = None;
            self.status = "Viewing main thread".into();
            return true;
        }
        let selector = selector.to_ascii_lowercase();
        let matches = self
            .view
            .team
            .sessions
            .iter()
            .filter(|session| {
                session.id.to_string() == selector
                    || format!("{:?}", session.kind).to_ascii_lowercase() == selector
                    || session.display_name.to_ascii_lowercase() == selector
            })
            .map(|session| session.id)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [session_id] => {
                self.selected_agent_session_id = Some(*session_id);
                self.transcript_scroll = 0;
                self.status = format!("Viewing {}", self.selected_agent_name());
                true
            }
            [] => {
                self.status = format!("No agent thread matches `{selector}`");
                true
            }
            _ => {
                self.status =
                    format!("Multiple agent threads match `{selector}`; choose one from /agent");
                true
            }
        }
    }

    fn agent_picker_candidates(&self) -> Vec<(Option<ConversationAgentSessionId>, String, String)> {
        let query = self
            .agent_picker
            .as_ref()
            .map_or("", |overlay| overlay.query.trim());
        let query = query.to_ascii_lowercase();
        let mut sessions = self.view.team.sessions.iter().collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            agent_session_state_rank(left.state)
                .cmp(&agent_session_state_rank(right.state))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        let mut candidates = vec![(
            None,
            "Main thread".to_owned(),
            "Owner-facing MetaCog conversation".to_owned(),
        )];
        candidates.extend(sessions.into_iter().map(|session| {
            let group = if agent_session_state_rank(session.state) == 0 {
                "Active"
            } else {
                "Done"
            };
            (
                Some(session.id),
                session.display_name.clone(),
                format!("{group} · {:?} · {:?}", session.kind, session.state),
            )
        }));
        if query.is_empty() {
            return candidates;
        }
        candidates
            .into_iter()
            .filter(|(_, label, detail)| {
                let searchable = format!("{label} {detail}").to_ascii_lowercase();
                searchable.contains(&query) || fuzzy_subsequence(&searchable, &query)
            })
            .collect()
    }

    fn handle_agent_picker_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<TuiAction>, Box<dyn std::error::Error>> {
        let candidates = self.agent_picker_candidates();
        let overlay = self.agent_picker.as_mut().expect("agent picker");
        overlay.selected = overlay.selected.min(candidates.len().saturating_sub(1));
        match key.code {
            KeyCode::Esc => {
                self.agent_picker = None;
                self.status = "Agent selection cancelled".into();
            }
            KeyCode::Up => overlay.selected = overlay.selected.saturating_sub(1),
            KeyCode::Down => {
                overlay.selected = (overlay.selected + 1).min(candidates.len().saturating_sub(1));
            }
            KeyCode::Backspace => {
                overlay.query.pop();
                overlay.selected = 0;
            }
            KeyCode::Char(value)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                overlay.query.push(value);
                overlay.selected = 0;
            }
            KeyCode::Enter if !candidates.is_empty() => {
                self.selected_agent_session_id = candidates[overlay.selected].0;
                self.transcript_scroll = 0;
                self.agent_picker = None;
                self.status = format!("Viewing {}", self.selected_agent_name());
            }
            _ => {}
        }
        Ok(None)
    }

    fn selected_agent_name(&self) -> String {
        self.selected_agent_session_id
            .and_then(|id| {
                self.view
                    .team
                    .sessions
                    .iter()
                    .find(|session| session.id == id)
            })
            .map_or_else(
                || "main thread".into(),
                |session| session.display_name.clone(),
            )
    }

    fn render(&mut self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        if self.cli_preferences.theme == "light" {
            frame.render_widget(
                Block::default().style(Style::default().fg(Color::Black).bg(Color::White)),
                area,
            );
        }
        let composer_height =
            u16::try_from(self.editor.buffer.lines().count().clamp(1, 6)).unwrap_or(1) + 4;
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(1),
                Constraint::Length(composer_height),
            ])
            .split(area);
        let transcript = Paragraph::new(self.transcript_text())
            .block(Block::default().borders(Borders::BOTTOM).title(
                if self.selected_agent_session_id.is_some() {
                    format!(
                        " MetaCog · {} · Agent: {} · Esc returns ",
                        self.view.conversation.title,
                        self.selected_agent_name()
                    )
                } else {
                    format!(" MetaCog · {} ", self.view.conversation.title)
                },
            ))
            .wrap(Wrap { trim: false })
            .scroll((self.transcript_scroll, 0));
        frame.render_widget(transcript, layout[0]);
        frame.render_widget(Paragraph::new(self.status_line()), layout[1]);
        let composer = Paragraph::new(self.editor.buffer.as_str())
            .style(
                Style::default().fg(if self.cli_preferences.theme == "light" {
                    Color::Black
                } else {
                    Color::White
                }),
            )
            .block(Block::default().borders(Borders::ALL).title(
                if self.selected_agent_session_id.is_some() {
                    " Message selected agent "
                } else if self.is_active() {
                    " Steer now · Tab queues "
                } else {
                    " Message MetaCog "
                },
            ))
            .wrap(Wrap { trim: false });
        frame.render_widget(composer, layout[2]);
        let cursor = cursor_position(layout[2], &self.editor.buffer, self.editor.cursor);
        frame.set_cursor_position(cursor);
        self.render_palette(frame, layout[2], area);
        self.render_history_search(frame, area);
        self.render_question_overlay(frame, area);
        self.render_agent_picker(frame, area);
    }

    fn transcript_text(&self) -> Text<'static> {
        if self.selected_agent_session_id.is_some() {
            return self.agent_transcript_text();
        }
        let mut entries = Vec::<(chrono::DateTime<chrono::Utc>, Line<'static>)>::new();
        for message in &self.view.messages {
            let (label, color) = match message.role {
                ConversationRole::User => ("You", Color::Cyan),
                ConversationRole::Assistant => ("MetaCog", Color::Green),
                ConversationRole::System => ("System", Color::Yellow),
            };
            entries.push((
                message.created_at,
                Line::from(vec![
                    Span::styled(
                        format!("{label}: "),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(message.content.clone()),
                ]),
            ));
        }
        for activity in &self.view.activities {
            if !self.details_expanded
                && matches!(
                    activity.kind,
                    ConversationActivityKind::Working | ConversationActivityKind::Usage
                )
            {
                continue;
            }
            let icon = match activity.kind {
                ConversationActivityKind::ReasoningSummary => "◇",
                ConversationActivityKind::Command => ">_",
                ConversationActivityKind::FileChange => "✎",
                ConversationActivityKind::Hook => "⌁",
                ConversationActivityKind::Approval => "!",
                ConversationActivityKind::Failure => "×",
                ConversationActivityKind::Completion => "✓",
                _ => "·",
            };
            let state = match activity.state {
                ConversationActivityState::Running => "running",
                ConversationActivityState::Completed => "done",
                ConversationActivityState::Failed => "failed",
                ConversationActivityState::Declined => "declined",
            };
            let detail = activity
                .command
                .as_deref()
                .or(activity.output.as_deref())
                .unwrap_or(state);
            let source = activity
                .agent_session_id
                .and_then(|id| {
                    self.view
                        .team
                        .sessions
                        .iter()
                        .find(|session| session.id == id)
                })
                .map_or_else(String::new, |session| {
                    format!("{} · ", session.display_name)
                });
            entries.push((
                activity.updated_at,
                Line::styled(
                    format!(
                        "  {icon} {source}{} · {detail}",
                        activity_kind_label(activity.kind)
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ));
            if self.cli_preferences.raw_mode {
                entries.push((
                    activity.updated_at,
                    Line::styled(
                        serde_json::to_string(activity).unwrap_or_else(|_| "{}".into()),
                        Style::default().fg(Color::Gray),
                    ),
                ));
            }
        }
        for (created_at, notice) in &self.notices {
            entries.push((
                *created_at,
                Line::from(vec![
                    Span::styled("MetaCog CLI: ", Style::default().fg(Color::Yellow)),
                    Span::raw(notice.clone()),
                ]),
            ));
        }
        if let Some(cleared_before) = self.transcript_cleared_before {
            entries.retain(|(created_at, _)| *created_at > cleared_before);
        }
        entries.sort_by_key(|entry| entry.0);
        let mut lines = Vec::new();
        for (_, line) in entries {
            lines.push(line);
            lines.push(Line::raw(""));
        }
        if lines.is_empty() {
            if let Some(pet) = &self.cli_preferences.pet {
                lines.push(Line::styled(
                    match pet.as_str() {
                        "ferris" => "(\u{2267}\u{25e1}\u{2266})",
                        "owl" => "(\u{25c9}v\u{25c9})",
                        other => other,
                    }
                    .to_owned(),
                    Style::default().fg(Color::Gray),
                ));
                lines.push(Line::raw(""));
            }
            lines.push(Line::styled(
                "What should we build?",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        Text::from(lines)
    }

    fn agent_transcript_text(&self) -> Text<'static> {
        let session_id = self.selected_agent_session_id.expect("selected agent");
        let session = self
            .view
            .team
            .sessions
            .iter()
            .find(|session| session.id == session_id);
        let mut entries = Vec::<(chrono::DateTime<chrono::Utc>, Line<'static>)>::new();
        if let Some(session) = session {
            entries.push((
                session.created_at,
                Line::styled(
                    format!(
                        "{} · {:?} · {:?} · {}",
                        session.display_name, session.kind, session.state, session.id
                    ),
                    Style::default()
                        .fg(match session.state {
                            ConversationAgentSessionState::Failed => Color::Red,
                            ConversationAgentSessionState::Working
                            | ConversationAgentSessionState::Starting => Color::Yellow,
                            _ => Color::Cyan,
                        })
                        .add_modifier(Modifier::BOLD),
                ),
            ));
        }
        for message in self.view.team.messages.iter().filter(|message| {
            message.from_session_id == session_id || message.to_session_id == session_id
        }) {
            let source = if message.tool_name.as_deref() == Some("owner_agent_message") {
                "You"
            } else {
                self.view
                    .team
                    .sessions
                    .iter()
                    .find(|candidate| candidate.id == message.from_session_id)
                    .map_or("Owner", |candidate| candidate.display_name.as_str())
            };
            let color = if message.from_session_id == session_id {
                Color::Green
            } else {
                Color::Cyan
            };
            entries.push((
                message.created_at,
                Line::from(vec![
                    Span::styled(
                        format!("{source}: "),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(message.content.clone()),
                ]),
            ));
        }
        for activity in self
            .view
            .activities
            .iter()
            .filter(|activity| activity.agent_session_id == Some(session_id))
        {
            let detail = activity
                .command
                .as_deref()
                .or(activity.output.as_deref())
                .unwrap_or_else(|| match activity.state {
                    ConversationActivityState::Running => "running",
                    ConversationActivityState::Completed => "done",
                    ConversationActivityState::Failed => "failed",
                    ConversationActivityState::Declined => "declined",
                });
            entries.push((
                activity.updated_at,
                Line::styled(
                    format!("  {} · {detail}", activity_kind_label(activity.kind)),
                    Style::default().fg(Color::DarkGray),
                ),
            ));
            if self.cli_preferences.raw_mode {
                entries.push((
                    activity.updated_at,
                    Line::styled(
                        serde_json::to_string(activity).unwrap_or_else(|_| "{}".into()),
                        Style::default().fg(Color::Gray),
                    ),
                ));
            }
        }
        entries.sort_by_key(|entry| entry.0);
        let mut lines = Vec::new();
        for (_, line) in entries {
            lines.push(line);
            lines.push(Line::raw(""));
        }
        if lines.is_empty() {
            lines.push(Line::styled(
                "No activity has been recorded in this agent thread.",
                Style::default().fg(Color::Gray),
            ));
        }
        Text::from(lines)
    }

    fn status_line(&self) -> Line<'static> {
        let prefs = &self.view.conversation.execution_preferences;
        let goal = self
            .view
            .conversation
            .goal
            .as_ref()
            .map_or("no goal".into(), |goal| format!("goal: {:?}", goal.status));
        let mut fields = Vec::new();
        if self.selected_agent_session_id.is_some() {
            fields.push(format!("agent: {}", self.selected_agent_name()));
        }
        for field in &self.cli_preferences.status_line {
            let value = match field.as_str() {
                "project" => self.project.clone(),
                "branch" => self.branch.clone(),
                "model" => prefs.model_id.as_deref().unwrap_or("Automatic").to_owned(),
                "effort" => prefs
                    .reasoning_effort
                    .as_deref()
                    .unwrap_or("Automatic")
                    .to_owned(),
                "permissions" => prefs.permission_profile.as_ref().map_or_else(
                    || format!("{:?}", prefs.approval_mode),
                    |profile| format!("profile:{profile}"),
                ),
                "goal" => goal.clone(),
                "usage" => "usage: /usage".into(),
                "queue" => format!("{} queued", self.queued.len()),
                _ => continue,
            };
            fields.push(value);
        }
        Line::from(vec![
            Span::styled(
                format!(" {} ", self.status),
                Style::default()
                    .fg(if self.is_active() {
                        Color::Yellow
                    } else {
                        Color::Green
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  {}", fields.join("  "))),
        ])
    }

    fn render_palette(&mut self, frame: &mut ratatui::Frame<'_>, composer: Rect, area: Rect) {
        if self.editor.palette_request().is_none() || self.candidates.is_empty() {
            return;
        }
        let height =
            u16::try_from(self.candidates.len().min(MAX_VISIBLE_CHOICES) + 2).unwrap_or(10);
        let width = area.width.saturating_sub(4).min(78).max(24);
        let y = composer.y.saturating_sub(height);
        let popup = Rect::new(composer.x.saturating_add(1), y, width, height);
        frame.render_widget(Clear, popup);
        let items = self
            .candidates
            .iter()
            .take(MAX_VISIBLE_CHOICES)
            .map(|candidate| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<22}", candidate.label()),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        candidate.detail().to_owned(),
                        Style::default().fg(Color::Gray),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Select "))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
        let mut state = ListState::default().with_selected(Some(self.editor.selected));
        frame.render_stateful_widget(list, popup, &mut state);
    }

    fn render_history_search(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(search) = &self.history_search else {
            return;
        };
        let popup = centered_rect(70, 5, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(search.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Search prompt history · Enter use · Esc cancel "),
            ),
            popup,
        );
    }

    fn render_question_overlay(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(overlay) = &self.question_overlay else {
            return;
        };
        let question = overlay.question();
        let options = question.options.as_deref().unwrap_or_default();
        let mut lines = vec![Line::styled(
            question.question.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )];
        lines.push(Line::raw(""));
        if options.is_empty() || overlay.other_mode {
            let value = if question.is_secret {
                "•".repeat(overlay.text.chars().count())
            } else {
                overlay.text.clone()
            };
            lines.push(Line::raw(if value.is_empty() {
                "Type an answer, then press Enter".into()
            } else {
                value
            }));
            if overlay.other_mode {
                lines.push(Line::styled(
                    "Esc returns to choices",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        } else {
            for (index, option) in options.iter().enumerate() {
                let prefix = if index == overlay.selected {
                    "›"
                } else {
                    " "
                };
                let recommended = if index == 0 { " · Recommended" } else { "" };
                lines.push(Line::styled(
                    format!(
                        "{prefix} {}. {}{recommended} — {}",
                        index + 1,
                        option.label,
                        option.description
                    ),
                    if index == overlay.selected {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                ));
            }
            if question.is_other {
                let index = options.len();
                lines.push(Line::styled(
                    format!(
                        "{} {}. Other answer",
                        if index == overlay.selected {
                            "›"
                        } else {
                            " "
                        },
                        index + 1
                    ),
                    if index == overlay.selected {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                ));
            }
        }
        let height = u16::try_from(lines.len() + 2)
            .unwrap_or(u16::MAX)
            .min(area.height);
        let popup = centered_rect(80, height.max(7), area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                Block::default().borders(Borders::ALL).title(format!(
                    " {} · question {} of {} ",
                    question.header,
                    overlay.question_index + 1,
                    overlay.questions.len()
                )),
            ),
            popup,
        );
    }

    fn render_agent_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(overlay) = &self.agent_picker else {
            return;
        };
        let candidates = self.agent_picker_candidates();
        let height = u16::try_from(candidates.len().min(MAX_VISIBLE_CHOICES) + 4)
            .unwrap_or(12)
            .min(area.height);
        let popup = centered_rect(76, height.max(7), area);
        frame.render_widget(Clear, popup);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(popup);
        frame.render_widget(
            Paragraph::new(overlay.query.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Switch agent thread · type to filter · Esc cancel "),
            ),
            rows[0],
        );
        let selected = overlay.selected.min(candidates.len().saturating_sub(1));
        let start = selected
            .saturating_sub(MAX_VISIBLE_CHOICES.saturating_sub(1))
            .min(candidates.len().saturating_sub(MAX_VISIBLE_CHOICES));
        let items = candidates
            .iter()
            .skip(start)
            .take(MAX_VISIBLE_CHOICES)
            .map(|(_, label, detail)| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{label:<24}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(detail.clone(), Style::default().fg(Color::Gray)),
                ]))
            })
            .collect::<Vec<_>>();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Threads "))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
        let mut state = ListState::default()
            .with_selected((!candidates.is_empty()).then_some(selected.saturating_sub(start)));
        frame.render_stateful_widget(list, rows[1], &mut state);
    }
}

fn activity_kind_label(kind: ConversationActivityKind) -> &'static str {
    match kind {
        ConversationActivityKind::Working => "Working",
        ConversationActivityKind::Waiting => "Waiting",
        ConversationActivityKind::ReasoningSummary => "Reasoning",
        ConversationActivityKind::Plan => "Plan",
        ConversationActivityKind::Command => "Command",
        ConversationActivityKind::FileChange => "File change",
        ConversationActivityKind::McpCall => "MCP",
        ConversationActivityKind::CollaborationCall => "Agent exchange",
        ConversationActivityKind::WebSearch => "Web search",
        ConversationActivityKind::ImageView => "Image",
        ConversationActivityKind::Approval => "Approval",
        ConversationActivityKind::Elicitation => "Question",
        ConversationActivityKind::ContextCompaction => "Compaction",
        ConversationActivityKind::Hook => "Hook",
        ConversationActivityKind::Usage => "Usage",
        ConversationActivityKind::Recovery => "Recovery",
        ConversationActivityKind::Interruption => "Interrupted",
        ConversationActivityKind::Failure => "Failure",
        ConversationActivityKind::Completion => "Completed",
    }
}

fn agent_session_state_rank(state: ConversationAgentSessionState) -> u8 {
    match state {
        ConversationAgentSessionState::Starting
        | ConversationAgentSessionState::Ready
        | ConversationAgentSessionState::Working
        | ConversationAgentSessionState::Waiting => 0,
        ConversationAgentSessionState::Failed | ConversationAgentSessionState::Retired => 1,
    }
}

fn cursor_position(area: Rect, value: &str, cursor: usize) -> (u16, u16) {
    let before = value.chars().take(cursor).collect::<String>();
    let mut lines = before.split('\n');
    let last = lines.next_back().unwrap_or_default();
    let row = u16::try_from(before.matches('\n').count()).unwrap_or(u16::MAX);
    let column = u16::try_from(last.chars().count()).unwrap_or(u16::MAX);
    (
        area.x.saturating_add(1).saturating_add(column),
        area.y
            .saturating_add(1)
            .saturating_add(row.min(area.height.saturating_sub(2))),
    )
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area
        .width
        .saturating_mul(percent_x)
        .saturating_div(100)
        .max(20);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}

pub fn prompt_for_question(
    question: &ConversationInputQuestion,
) -> Result<String, Box<dyn std::error::Error>> {
    let options = question.options.as_deref().unwrap_or_default();
    if options.is_empty() {
        return prompt_for_text(question);
    }

    let mut stdout = io::stdout();
    writeln!(stdout, "\n{}\n{}", question.header, question.question)?;
    for (index, option) in options.iter().enumerate() {
        let recommended = if index == 0 { " (Recommended)" } else { "" };
        writeln!(
            stdout,
            "  {}. {}{}{}",
            index + 1,
            option.label,
            recommended,
            if option.description.is_empty() {
                String::new()
            } else {
                format!(" — {}", option.description)
            }
        )?;
    }
    if question.is_other {
        writeln!(stdout, "  {}. Other answer", options.len() + 1)?;
    }
    write!(stdout, "Choose [1]: ")?;
    stdout.flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    let selected = if answer.is_empty() {
        1
    } else {
        answer
            .parse::<usize>()
            .map_err(|_| "enter the number of an available choice")?
    };
    if selected >= 1 && selected <= options.len() {
        return Ok(options[selected - 1].label.clone());
    }
    if question.is_other && selected == options.len() + 1 {
        return prompt_for_text(question);
    }
    Err("the selected choice is out of range".into())
}

fn prompt_for_text(
    question: &ConversationInputQuestion,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    if question.options.as_ref().is_none_or(Vec::is_empty) {
        writeln!(stdout, "\n{}\n{}", question.header, question.question)?;
    }
    let answer = if question.is_secret {
        rpassword::prompt_password("Answer: ")?
    } else {
        write!(stdout, "Answer: ")?;
        stdout.flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        answer
    };
    let answer = answer.trim().to_owned();
    if answer.is_empty() {
        return Err("an answer is required".into());
    }
    Ok(answer)
}

#[derive(Default)]
struct EditorState {
    buffer: String,
    cursor: usize,
    selected: usize,
    palette_dismissed: bool,
    mentions: Vec<SelectedMention>,
}

impl EditorState {
    fn chars(&self) -> Vec<char> {
        self.buffer.chars().collect()
    }

    fn replace_all(&mut self, value: String) {
        self.buffer = value;
        self.cursor = self.buffer.chars().count();
        self.selected = 0;
        self.palette_dismissed = false;
        self.mentions.clear();
    }

    fn edit(&mut self, start: usize, end: usize, replacement: &str) {
        let mut chars = self.chars();
        let replacement_chars = replacement.chars().collect::<Vec<_>>();
        chars.splice(start..end, replacement_chars.iter().copied());
        self.buffer = chars.into_iter().collect();
        let removed = end.saturating_sub(start);
        let inserted = replacement_chars.len();
        self.cursor = start + inserted;
        self.selected = 0;
        self.palette_dismissed = false;
        self.mentions.retain_mut(|mention| {
            if start < mention.end && end > mention.start
                || (start == end && start > mention.start && start < mention.end)
            {
                return false;
            }
            if mention.start >= end {
                if inserted >= removed {
                    let delta = inserted - removed;
                    mention.start += delta;
                    mention.end += delta;
                } else {
                    let delta = removed - inserted;
                    mention.start -= delta;
                    mention.end -= delta;
                }
            }
            true
        });
    }

    fn delete_backward(&mut self) {
        if let Some(mention) = self
            .mentions
            .iter()
            .find(|mention| self.cursor > mention.start && self.cursor <= mention.end)
            .cloned()
        {
            self.edit(mention.start, mention.end, "");
        } else {
            self.edit(self.cursor - 1, self.cursor, "");
        }
    }

    fn delete_forward(&mut self) {
        if let Some(mention) = self
            .mentions
            .iter()
            .find(|mention| self.cursor >= mention.start && self.cursor < mention.end)
            .cloned()
        {
            self.edit(mention.start, mention.end, "");
        } else {
            self.edit(self.cursor, self.cursor + 1, "");
        }
    }

    fn palette_request(&self) -> Option<PaletteRequest> {
        if self.palette_dismissed {
            return None;
        }
        let chars = self.chars();
        if self.cursor <= chars.len() && chars.first() == Some(&'/') {
            let prefix = chars[1..self.cursor].iter().collect::<String>();
            if !prefix.chars().any(char::is_whitespace) {
                return Some(PaletteRequest {
                    kind: PaletteKind::Slash,
                    query: prefix,
                    command: None,
                });
            }
            let input = chars[1..self.cursor].iter().collect::<String>();
            if let Some((command, query)) = input.split_once(char::is_whitespace) {
                let query = query.trim_start();
                if command == "mention" {
                    return Some(PaletteRequest {
                        kind: PaletteKind::Mention,
                        query: query.to_owned(),
                        command: Some(command.to_owned()),
                    });
                }
                if [
                    "model",
                    "permissions",
                    "personality",
                    "resume",
                    "agent",
                    "subagents",
                    "stop",
                    "theme",
                    "keymap",
                    "statusline",
                    "title",
                    "apps",
                    "hooks",
                ]
                .contains(&command)
                {
                    return Some(PaletteRequest {
                        kind: PaletteKind::CommandOption,
                        query: query.to_owned(),
                        command: Some(command.to_owned()),
                    });
                }
            }
        }
        let mut start = self.cursor;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        if chars.get(start) == Some(&'@')
            && (start == 0
                || chars
                    .get(start.wrapping_sub(1))
                    .is_none_or(|value| value.is_whitespace()))
        {
            return Some(PaletteRequest {
                kind: PaletteKind::Mention,
                query: chars[start + 1..self.cursor].iter().collect(),
                command: None,
            });
        }
        if chars.get(start) == Some(&'$')
            && (start == 0
                || chars
                    .get(start.wrapping_sub(1))
                    .is_none_or(|value| value.is_whitespace()))
        {
            return Some(PaletteRequest {
                kind: PaletteKind::Dollar,
                query: chars[start + 1..self.cursor].iter().collect(),
                command: None,
            });
        }
        None
    }

    fn active_range(&self) -> Option<(usize, usize)> {
        let request = self.palette_request()?;
        match request.kind {
            PaletteKind::Slash => Some((0, self.cursor)),
            PaletteKind::Mention | PaletteKind::Dollar => {
                let chars = self.chars();
                let mut start = self.cursor;
                while start > 0 && !chars[start - 1].is_whitespace() {
                    start -= 1;
                }
                Some((start, self.cursor))
            }
            PaletteKind::CommandOption => {
                let chars = self.chars();
                let mut start = self.cursor;
                while start > 0 && !chars[start - 1].is_whitespace() {
                    start -= 1;
                }
                Some((start, self.cursor))
            }
        }
    }

    fn select(&mut self, candidate: &PaletteCandidate) {
        let request = self.palette_request();
        let Some((start, end)) = self.active_range() else {
            return;
        };
        match candidate {
            PaletteCandidate::Slash(candidate) => {
                self.edit(start, end, &format!("/{} ", candidate.name));
            }
            PaletteCandidate::Mention(candidate) => {
                let replacement = format!("@{}", candidate.label);
                self.edit(start, end, &replacement);
                let mention_end = start + replacement.chars().count();
                self.mentions.push(SelectedMention {
                    start,
                    end: mention_end,
                    candidate: candidate.clone(),
                });
                if self.cursor == self.chars().len() {
                    self.edit(self.cursor, self.cursor, " ");
                }
            }
            PaletteCandidate::Reference { value, .. } => {
                self.edit(start, end, &format!("${value}"));
                if self.cursor == self.chars().len() {
                    self.edit(self.cursor, self.cursor, " ");
                }
            }
            PaletteCandidate::Choice { value, .. } => {
                if request
                    .as_ref()
                    .and_then(|request| request.command.as_deref())
                    == Some("apps")
                {
                    self.edit(0, end, &format!("${value} "));
                } else {
                    self.edit(start, end, value);
                }
            }
        }
        self.palette_dismissed = true;
    }

    fn dismiss_palette(&mut self) {
        self.palette_dismissed = true;
        self.selected = 0;
    }

    fn clamp_selection(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn submit(self) -> Result<SubmittedLine, Box<dyn std::error::Error>> {
        let chars = self.chars();
        let mut mentions = self.mentions;
        mentions.sort_by_key(|mention| mention.start);
        let inputs = mentions
            .into_iter()
            .map(|mention| {
                let byte_start = chars[..mention.start].iter().collect::<String>().len();
                let byte_end = chars[..mention.end].iter().collect::<String>().len();
                Ok(ComposerMentionInput {
                    kind: mention.candidate.kind,
                    label: mention.candidate.label,
                    resource: mention.candidate.resource,
                    byte_start: u32::try_from(byte_start)?,
                    byte_end: u32::try_from(byte_end)?,
                    plugin_revision_id: mention.candidate.plugin_revision_id,
                    installation_revision: mention.candidate.installation_revision,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        Ok(SubmittedLine {
            content: self.buffer,
            mentions: inputs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mcg_domain::{
        CONVERSATION_TEAM_SCHEMA_VERSION, Conversation, ConversationAgentKind,
        ConversationAgentSession, ConversationAgentSessionState, ConversationInputOption,
        ConversationInputRequest, ConversationTeamState, ConversationTurn, ConversationTurnId,
        ExecutionPreferences, MentionKind, OrganizationRevisionId, RoleId,
    };

    fn mention(label: &str, resource: &str) -> PaletteCandidate {
        PaletteCandidate::Mention(MentionCandidate {
            kind: MentionKind::File,
            label: label.to_owned(),
            resource: resource.to_owned(),
            detail: None,
            plugin_revision_id: None,
            installation_revision: None,
        })
    }

    #[test]
    fn session_picker_searches_titles_projects_and_ids_with_fuzzy_fallback() {
        let candidates = vec![
            SessionPickerCandidate {
                id: "first-id".into(),
                title: "Release verification".into(),
                detail: "MetaCog · interactive".into(),
            },
            SessionPickerCandidate {
                id: "second-id".into(),
                title: "Structured output".into(),
                detail: "ExampleSite · non-interactive".into(),
            },
        ];
        assert_eq!(fuzzy_session_indices(&candidates, "structured"), vec![1]);
        assert_eq!(fuzzy_session_indices(&candidates, "examplesite"), vec![1]);
        assert_eq!(fuzzy_session_indices(&candidates, "rlvrf"), vec![0]);
        assert_eq!(fuzzy_session_indices(&candidates, "second-id"), vec![1]);
    }

    #[test]
    fn session_picker_renders_at_compact_and_wide_terminal_sizes() {
        let candidates = (0..20)
            .map(|index| SessionPickerCandidate {
                id: format!("conversation-{index}"),
                title: format!("Conversation {index}"),
                detail: "Project · interactive · 2026-08-28 22:00 UTC".into(),
            })
            .collect::<Vec<_>>();
        let filtered = fuzzy_session_indices(&candidates, "conversation");
        for (width, height) in [(48, 12), (96, 18), (180, 30)] {
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| {
                    render_session_picker(
                        frame,
                        &candidates,
                        &filtered,
                        "conversation",
                        19,
                        "Resume a conversation",
                        "Search titles, ids, and projects",
                        "No matching conversations · Esc cancels",
                        "↑↓ select · Enter resume · type to filter · Esc cancel",
                    );
                })
                .expect("picker should render without clipping or panics");
        }
    }

    #[test]
    fn palettes_only_open_for_slash_prefixes_and_at_token_boundaries() {
        let mut state = EditorState::default();
        state.replace_all("/pla".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::Slash,
                query: "pla".into(),
                command: None,
            })
        );
        state.replace_all("/model ter".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::CommandOption,
                query: "ter".into(),
                command: Some("model".into()),
            })
        );
        state.replace_all("/hooks ".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::CommandOption,
                query: String::new(),
                command: Some("hooks".into()),
            })
        );
        state.replace_all("email@example.com".into());
        assert_eq!(state.palette_request(), None);
        state.replace_all("inspect @src".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::Mention,
                query: "src".into(),
                command: None,
            })
        );
    }

    #[test]
    fn selection_creates_valid_utf8_byte_spans_and_editing_tokens_invalidates_them() {
        let mut state = EditorState::default();
        state.replace_all("Résumé @ma".into());
        state.select(&mention("main.rs", "src/main.rs"));
        let submitted = state.submit().expect("submission");
        assert_eq!(submitted.content, "Résumé @main.rs ");
        assert_eq!(submitted.mentions.len(), 1);
        let selected = &submitted.mentions[0];
        assert_eq!(
            &submitted.content[selected.byte_start as usize..selected.byte_end as usize],
            "@main.rs"
        );

        let mut edited = EditorState::default();
        edited.replace_all("@ma".into());
        edited.select(&mention("main.rs", "src/main.rs"));
        edited.edit(2, 3, "x");
        assert!(edited.submit().expect("submission").mentions.is_empty());
    }

    #[test]
    fn slash_selection_inserts_command_and_argument_space() {
        let mut state = EditorState::default();
        state.replace_all("/go".into());
        state.select(&PaletteCandidate::Slash(SlashCandidate {
            name: "goal",
            description: "Set a goal",
        }));
        assert_eq!(state.buffer, "/goal ");
        assert!(state.mentions.is_empty());
    }

    #[test]
    fn app_picker_selection_replaces_the_command_with_a_dollar_reference() {
        let mut state = EditorState::default();
        state.replace_all("/apps cal".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::CommandOption,
                query: "cal".into(),
                command: Some("apps".into()),
            })
        );
        state.select(&PaletteCandidate::Choice {
            value: "calendar".into(),
            label: "Calendar".into(),
            detail: "Enabled app".into(),
        });
        assert_eq!(state.buffer, "$calendar ");
    }

    #[test]
    fn backspace_and_delete_remove_whole_mention_tokens() {
        let mut backward = EditorState::default();
        backward.replace_all("@ma".into());
        backward.select(&mention("main.rs", "src/main.rs"));
        backward.cursor -= 1;
        backward.delete_backward();
        assert_eq!(backward.buffer, " ");
        assert!(backward.mentions.is_empty());

        let mut forward = EditorState::default();
        forward.replace_all("@ma".into());
        forward.select(&mention("main.rs", "src/main.rs"));
        forward.cursor = 0;
        forward.delete_forward();
        assert_eq!(forward.buffer, " ");
        assert!(forward.mentions.is_empty());
    }

    fn view(active: bool) -> ConversationView {
        let conversation = Conversation::new("CLI", None);
        let turns = active
            .then(|| ConversationTurn {
                schema_version: 1,
                id: ConversationTurnId::new(),
                conversation_id: conversation.id,
                state: ConversationTurnState::Streaming,
                execution_preferences: ExecutionPreferences::default(),
                codex_thread_id: Some("thread".into()),
                codex_turn_id: Some("turn".into()),
                task_id: None,
                run_id: None,
                error: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .into_iter()
            .collect();
        ConversationView {
            conversation,
            turns,
            messages: Vec::new(),
            activities: Vec::new(),
            team: ConversationTeamState::default(),
        }
    }

    fn agent_session(
        conversation: &Conversation,
        display_name: &str,
        kind: ConversationAgentKind,
        state: ConversationAgentSessionState,
    ) -> ConversationAgentSession {
        ConversationAgentSession {
            schema_version: CONVERSATION_TEAM_SCHEMA_VERSION,
            id: ConversationAgentSessionId::new(),
            conversation_id: conversation.id,
            role_id: RoleId::new(),
            organization_revision_id: OrganizationRevisionId::new(),
            kind,
            display_name: display_name.into(),
            parent_session_id: None,
            state,
            codex_thread_id: Some(format!("thread-{display_name}")),
            model: Some("gpt-5.6-terra".into()),
            prompt_revision: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn agent_picker_filters_active_first_and_switches_the_transcript_view() {
        let mut agent_view = view(false);
        let retired = agent_session(
            &agent_view.conversation,
            "Retired Coder",
            ConversationAgentKind::Coder,
            ConversationAgentSessionState::Retired,
        );
        let working = agent_session(
            &agent_view.conversation,
            "Coding Agent Atlas",
            ConversationAgentKind::Coder,
            ConversationAgentSessionState::Working,
        );
        agent_view.team.sessions = vec![retired, working.clone()];
        let mut model = TuiModel::new(agent_view, "project".into(), "main".into());

        model.editor.replace_all("/agent".into());
        model.set_candidates(vec![PaletteCandidate::Slash(SlashCandidate {
            name: "agent",
            description: "Switch agent thread",
        })]);
        assert!(
            model
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .expect("open picker")
                .is_none()
        );
        let candidates = model.agent_picker_candidates();
        assert_eq!(candidates[0].1, "Main thread");
        assert_eq!(candidates[1].1, "Coding Agent Atlas");
        assert!(candidates[1].2.starts_with("Active"));
        assert_eq!(candidates[2].1, "Retired Coder");
        assert!(candidates[2].2.starts_with("Done"));

        model
            .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("choose active agent");
        model
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("select active agent");
        assert_eq!(model.selected_agent_session_id, Some(working.id));
        assert!(model.agent_picker.is_none());
        assert_eq!(model.selected_agent_name(), "Coding Agent Atlas");

        model.editor.replace_all("Focus on the failing test".into());
        assert!(matches!(
            model
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .expect("route agent message"),
            Some(TuiAction::MessageAgent { session_id, content })
                if session_id == working.id && content == "Focus on the failing test"
        ));

        assert!(
            model
                .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                .expect("return to main")
                .is_none()
        );
        assert!(model.selected_agent_session_id.is_none());
    }

    #[test]
    fn global_agent_browser_can_focus_only_a_session_in_the_selected_conversation() {
        let mut agent_view = view(false);
        let selected = agent_session(
            &agent_view.conversation,
            "Tester",
            ConversationAgentKind::Tester,
            ConversationAgentSessionState::Working,
        );
        let absent = ConversationAgentSessionId::new();
        agent_view.team.sessions.push(selected.clone());
        let mut model = TuiModel::new(agent_view, "project".into(), "main".into());

        assert!(!model.focus_agent_session(absent));
        assert!(model.selected_agent_session_id.is_none());
        assert!(model.focus_agent_session(selected.id));
        assert_eq!(model.selected_agent_session_id, Some(selected.id));
        assert_eq!(model.selected_agent_name(), "Tester");
    }

    #[test]
    fn agent_picker_scrolls_to_late_choices_at_compact_and_wide_sizes() {
        let mut agent_view = view(false);
        agent_view.team.sessions = (0..20)
            .map(|index| {
                agent_session(
                    &agent_view.conversation,
                    &format!("Coding Agent {index}"),
                    ConversationAgentKind::Coder,
                    ConversationAgentSessionState::Ready,
                )
            })
            .collect();
        let mut model = TuiModel::new(agent_view, "project".into(), "main".into());
        model.agent_picker = Some(AgentPickerOverlay {
            query: String::new(),
            selected: 20,
        });
        for (width, height) in [(48, 12), (96, 18), (180, 30)] {
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| model.render(frame))
                .expect("agent picker should render without clipping or panics");
        }
    }

    #[test]
    fn enter_submits_idle_and_steers_active_while_tab_queues() {
        let mut idle = TuiModel::new(view(false), "project".into(), "main".into());
        idle.editor.replace_all("build it".into());
        assert!(matches!(
            idle.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .expect("idle"),
            Some(TuiAction::Submit(_))
        ));

        let mut active = TuiModel::new(view(true), "project".into(), "main".into());
        active.editor.replace_all("use blue".into());
        assert!(matches!(
            active
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .expect("steer"),
            Some(TuiAction::Steer(_))
        ));

        let mut queued = TuiModel::new(view(true), "project".into(), "main".into());
        queued.editor.replace_all("then test".into());
        assert!(matches!(
            queued
                .handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
                .expect("queue"),
            Some(TuiAction::Queue(_))
        ));
    }

    #[test]
    fn shift_enter_inserts_a_multiline_break() {
        let mut model = TuiModel::new(view(false), "project".into(), "main".into());
        model.editor.replace_all("first".into());
        assert!(
            model
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT))
                .expect("newline")
                .is_none()
        );
        assert_eq!(model.editor.buffer, "first\n");
    }

    #[test]
    fn control_l_clears_only_the_rendered_transcript() {
        let mut model = TuiModel::new(view(false), "project".into(), "main".into());
        model.notices.push_back((
            Utc::now() - chrono::Duration::seconds(1),
            "old notice".into(),
        ));
        assert!(
            model
                .transcript_text()
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content.contains("old notice"))
        );

        assert!(
            model
                .handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL))
                .expect("clear display")
                .is_none()
        );
        assert!(
            !model
                .transcript_text()
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content.contains("old notice"))
        );
        assert!(model.view.messages.is_empty());
        assert!(model.status.contains("history is retained"));
    }

    #[test]
    fn planning_questions_use_a_non_blocking_recommended_choice_overlay() {
        let mut question_view = view(true);
        question_view
            .conversation
            .pending_input_requests
            .push(ConversationInputRequest {
                request_id: serde_json::json!({ "id": "request-1" }),
                thread_id: "thread".into(),
                turn_id: "turn".into(),
                item_id: "item".into(),
                questions: vec![ConversationInputQuestion {
                    id: "scope".into(),
                    header: "Scope".into(),
                    question: "How broad should this be?".into(),
                    is_other: true,
                    is_secret: false,
                    options: Some(vec![
                        ConversationInputOption {
                            label: "Focused".into(),
                            description: "Smallest useful change".into(),
                        },
                        ConversationInputOption {
                            label: "Broad".into(),
                            description: "Wider refactor".into(),
                        },
                    ]),
                }],
                received_at: Utc::now(),
            });
        let mut model = TuiModel::new(question_view.clone(), "project".into(), "main".into());
        model.set_view(question_view);
        let action = model
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("question response")
            .expect("answer action");
        let TuiAction::AnswerInput {
            request_id,
            answers,
        } = action
        else {
            panic!("expected an input answer");
        };
        assert_eq!(request_id, serde_json::json!({ "id": "request-1" }));
        assert_eq!(
            answers,
            serde_json::json!({ "scope": { "answers": ["Focused"] } })
        );
    }

    #[test]
    fn terminal_layout_renders_without_clipping_panics_at_compact_and_wide_sizes() {
        for (width, height) in [(40, 12), (120, 40)] {
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
            let mut model = TuiModel::new(view(false), "project".into(), "main".into());
            model.editor.replace_all("first line\nsecond line".into());
            terminal
                .draw(|frame| model.render(frame))
                .expect("render should fit terminal");
            let rendered = terminal.backend().buffer().content();
            assert_eq!(rendered.len(), usize::from(width) * usize::from(height));
        }
    }
}
