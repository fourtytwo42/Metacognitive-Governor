use std::{
    future::Future,
    io::{self, Write},
};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, ClearType},
};
use mcg_domain::{ComposerMentionInput, ConversationInputQuestion, MentionCandidate};

const PROMPT: &str = "metacog> ";
const MAX_VISIBLE_CHOICES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteKind {
    Slash,
    Mention,
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
}

impl PaletteCandidate {
    fn label(&self) -> String {
        match self {
            Self::Slash(candidate) => format!("/{}", candidate.name),
            Self::Mention(candidate) => format!("@{}", candidate.label),
        }
    }

    fn detail(&self) -> &str {
        match self {
            Self::Slash(candidate) => candidate.description,
            Self::Mention(candidate) => candidate.detail.as_deref().unwrap_or(&candidate.resource),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteRequest {
    pub kind: PaletteKind,
    pub query: String,
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
pub enum ComposerOutcome {
    Submit(SubmittedLine),
    Eof,
}

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Default)]
pub struct Composer {
    history: Vec<String>,
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

impl Composer {
    pub async fn read_line<F, Fut>(
        &mut self,
        mut candidates_for: F,
    ) -> Result<ComposerOutcome, Box<dyn std::error::Error>>
    where
        F: FnMut(PaletteRequest) -> Fut,
        Fut: Future<Output = Vec<PaletteCandidate>>,
    {
        let _raw_mode = RawModeGuard::enter()?;
        let mut state = EditorState::default();
        let mut history_index = self.history.len();
        let mut stdout = io::stdout();

        loop {
            let request = state.palette_request();
            let candidates = request.clone().map(&mut candidates_for);
            let candidates = if let Some(candidates) = candidates {
                candidates.await
            } else {
                Vec::new()
            };
            state.clamp_selection(candidates.len());
            render(&mut stdout, &state, &candidates)?;

            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == event::KeyEventKind::Release {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('c') => {
                        clear_render(&mut stdout)?;
                        execute!(stdout, Print("^C\r\n"))?;
                        return Ok(ComposerOutcome::Eof);
                    }
                    KeyCode::Char('d') if state.buffer.is_empty() => {
                        clear_render(&mut stdout)?;
                        execute!(stdout, Print("\r\n"))?;
                        return Ok(ComposerOutcome::Eof);
                    }
                    _ => {}
                }
            }

            match key.code {
                KeyCode::Enter | KeyCode::Tab if request.is_some() && !candidates.is_empty() => {
                    state.select(&candidates[state.selected]);
                }
                KeyCode::Enter => {
                    let submitted = state.submit()?;
                    clear_render(&mut stdout)?;
                    execute!(stdout, Print(format!("{PROMPT}{}\r\n", submitted.content)))?;
                    if !submitted.content.trim().is_empty() {
                        self.history.push(submitted.content.clone());
                    }
                    return Ok(ComposerOutcome::Submit(submitted));
                }
                KeyCode::Esc => state.dismiss_palette(),
                KeyCode::Up if request.is_some() && !candidates.is_empty() => {
                    state.selected = state.selected.saturating_sub(1);
                }
                KeyCode::Down if request.is_some() && !candidates.is_empty() => {
                    state.selected = (state.selected + 1).min(candidates.len() - 1);
                }
                KeyCode::Up => {
                    if history_index > 0 {
                        history_index -= 1;
                        state.replace_all(self.history[history_index].clone());
                    }
                }
                KeyCode::Down => {
                    if history_index + 1 < self.history.len() {
                        history_index += 1;
                        state.replace_all(self.history[history_index].clone());
                    } else if history_index < self.history.len() {
                        history_index = self.history.len();
                        state.replace_all(String::new());
                    }
                }
                KeyCode::Left => state.cursor = state.cursor.saturating_sub(1),
                KeyCode::Right => state.cursor = (state.cursor + 1).min(state.chars().len()),
                KeyCode::Home => state.cursor = 0,
                KeyCode::End => state.cursor = state.chars().len(),
                KeyCode::Backspace if state.cursor > 0 => {
                    state.delete_backward();
                }
                KeyCode::Delete if state.cursor < state.chars().len() => {
                    state.delete_forward();
                }
                KeyCode::Char(value)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    state.edit(state.cursor, state.cursor, &value.to_string());
                }
                _ => {}
            }
        }
    }
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
                });
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
            });
        }
        None
    }

    fn active_range(&self) -> Option<(usize, usize)> {
        let request = self.palette_request()?;
        match request.kind {
            PaletteKind::Slash => Some((0, self.cursor)),
            PaletteKind::Mention => {
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

fn clear_render(stdout: &mut io::Stdout) -> io::Result<()> {
    execute!(
        stdout,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::FromCursorDown)
    )
}

fn render(
    stdout: &mut io::Stdout,
    state: &EditorState,
    candidates: &[PaletteCandidate],
) -> io::Result<()> {
    clear_render(stdout)?;
    let width = terminal::size().map_or(100, |(width, _)| usize::from(width));
    let available = width.saturating_sub(PROMPT.len() + 1).max(12);
    let chars = state.chars();
    let window_start = state.cursor.saturating_sub(available);
    let window_end = (window_start + available).min(chars.len());
    let visible = chars[window_start..window_end].iter().collect::<String>();
    execute!(stdout, Print(PROMPT), Print(visible))?;

    let visible_candidates = candidates
        .iter()
        .take(MAX_VISIBLE_CHOICES)
        .collect::<Vec<_>>();
    for (index, candidate) in visible_candidates.iter().enumerate() {
        execute!(stdout, Print("\r\n"))?;
        if index == state.selected {
            execute!(stdout, SetAttribute(Attribute::Reverse))?;
        }
        let label = candidate.label();
        let detail = candidate.detail();
        let max_detail = width.saturating_sub(label.len() + 8);
        let detail = detail.chars().take(max_detail).collect::<String>();
        execute!(stdout, Print(format!("  {label:<24} {detail}")))?;
        if index == state.selected {
            execute!(stdout, SetAttribute(Attribute::Reset))?;
        }
    }
    if candidates.len() > MAX_VISIBLE_CHOICES {
        execute!(
            stdout,
            Print(format!(
                "\r\n  … {} more matches; keep typing to narrow",
                candidates.len() - MAX_VISIBLE_CHOICES
            ))
        )?;
    }
    let rows = u16::try_from(
        visible_candidates.len() + usize::from(candidates.len() > MAX_VISIBLE_CHOICES),
    )
    .unwrap_or(u16::MAX);
    if rows > 0 {
        execute!(stdout, cursor::MoveUp(rows))?;
    }
    let cursor_column =
        u16::try_from(PROMPT.len() + state.cursor.saturating_sub(window_start)).unwrap_or(u16::MAX);
    execute!(stdout, cursor::MoveToColumn(cursor_column))?;
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::MentionKind;

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
    fn palettes_only_open_for_slash_prefixes_and_at_token_boundaries() {
        let mut state = EditorState::default();
        state.replace_all("/pla".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::Slash,
                query: "pla".into()
            })
        );
        state.replace_all("email@example.com".into());
        assert_eq!(state.palette_request(), None);
        state.replace_all("inspect @src".into());
        assert_eq!(
            state.palette_request(),
            Some(PaletteRequest {
                kind: PaletteKind::Mention,
                query: "src".into()
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
}
