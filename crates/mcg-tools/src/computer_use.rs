use std::{collections::HashMap, io::Cursor};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use mcg_domain::{ComputerUseCapability, ComputerUseRegion, ComputerUseTarget, PlatformSessionId};
use screenshots::{Screen, image::ImageOutputFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::ToolError;

const MAX_ACTIONS: usize = 32;
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_FRAME_BYTES: usize = 32 * 1_024 * 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerMouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerKey {
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Space,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ComputerUseAction {
    Move {
        x: u32,
        y: u32,
    },
    Click {
        x: u32,
        y: u32,
        button: ComputerMouseButton,
    },
    Scroll {
        x: u32,
        y: u32,
        delta_x: i32,
        delta_y: i32,
    },
    TypeText {
        text: String,
    },
    Key {
        key: ComputerKey,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerUseObservation {
    pub session_id: PlatformSessionId,
    pub display_id: u32,
    pub region: ComputerUseRegion,
    pub capabilities: Vec<ComputerUseCapability>,
    pub handed_off: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerUseFrame {
    pub observation: ComputerUseObservation,
    pub sha256: String,
    pub media_type: String,
    pub png_base64: String,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerUseBatchResult {
    pub prior_frame_sha256: String,
    pub actions_executed: u16,
    pub frame: ComputerUseFrame,
}

#[derive(Debug, Clone)]
struct AttachedComputer {
    display_id: u32,
    display_x: i32,
    display_y: i32,
    region: ComputerUseRegion,
    capabilities: Vec<ComputerUseCapability>,
    handed_off: bool,
}

#[derive(Debug, Default)]
pub struct ComputerUseManager {
    sessions: Mutex<HashMap<PlatformSessionId, AttachedComputer>>,
}

impl ComputerUseManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves and binds one display region before durable authority becomes active.
    ///
    /// # Errors
    /// Returns an error for invalid grants, an unavailable target, or duplicate attachment.
    pub async fn attach(
        &self,
        session_id: PlatformSessionId,
        target: &ComputerUseTarget,
        capabilities: &[ComputerUseCapability],
    ) -> Result<ComputerUseObservation, ToolError> {
        validate_capabilities(capabilities)?;
        let target = target.clone();
        let attached = tokio::task::spawn_blocking(move || resolve_target(&target))
            .await
            .map_err(|error| {
                ToolError::Execution(format!("computer attach worker failed: {error}"))
            })??;
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(&session_id) {
            return Err(ToolError::Invalid(
                "computer session is already attached".into(),
            ));
        }
        let attached = AttachedComputer {
            capabilities: capabilities.to_vec(),
            ..attached
        };
        let observation = observation(session_id, &attached);
        sessions.insert(session_id, attached);
        Ok(observation)
    }

    /// Returns the attached target without capturing pixels.
    ///
    /// # Errors
    /// Returns an error when the session is absent or handed to the local owner.
    pub async fn observe(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<ComputerUseObservation, ToolError> {
        let sessions = self.sessions.lock().await;
        let attached = sessions
            .get(&session_id)
            .ok_or_else(|| ToolError::Invalid("computer session is not attached".into()))?;
        if attached.handed_off {
            return Err(ToolError::Denied(
                "computer observations are unavailable while control is handed to the local owner"
                    .into(),
            ));
        }
        Ok(observation(session_id, attached))
    }

    /// Captures the region currently held by MCG authority.
    ///
    /// # Errors
    /// Returns an error when authority, capability, display capture, or encoding fails.
    pub async fn capture(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<ComputerUseFrame, ToolError> {
        let sessions = self.sessions.lock().await;
        let attached = owned_attached(&sessions, session_id)?;
        require_capability(&attached, ComputerUseCapability::ScreenRead)?;
        let result = capture_frame(session_id, attached).await;
        drop(sessions);
        result
    }

    /// Executes validated input against an unchanged observed frame.
    ///
    /// # Errors
    /// Returns an error when validation, authority, capture, or host input fails.
    pub async fn apply(
        &self,
        session_id: PlatformSessionId,
        expected_frame_sha256: &str,
        actions: &[ComputerUseAction],
    ) -> Result<ComputerUseBatchResult, ToolError> {
        validate_actions(actions)?;
        let sessions = self.sessions.lock().await;
        let attached = owned_attached(&sessions, session_id)?;
        require_capability(&attached, ComputerUseCapability::ScreenRead)?;
        let before = capture_frame(session_id, attached.clone()).await?;
        if before.sha256 != expected_frame_sha256 {
            return Err(ToolError::Denied(
                "computer frame changed after observation; capture a fresh frame before acting"
                    .into(),
            ));
        }
        validate_action_capabilities(&attached, actions)?;
        let execution_target = attached.clone();
        let owned_actions = actions.to_vec();
        tokio::task::spawn_blocking(move || execute_actions(&execution_target, &owned_actions))
            .await
            .map_err(|error| {
                ToolError::Execution(format!("computer input worker failed: {error}"))
            })??;
        let frame = capture_frame(session_id, attached).await?;
        let result = ComputerUseBatchResult {
            prior_frame_sha256: before.sha256,
            actions_executed: u16::try_from(actions.len())
                .map_err(|_| ToolError::Invalid("too many computer actions".into()))?,
            frame,
        };
        drop(sessions);
        Ok(result)
    }

    /// Makes the attached target unavailable to MCG until explicit reclaim.
    ///
    /// # Errors
    /// Returns an error when the session is absent or already handed off.
    pub async fn handoff(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<ComputerUseObservation, ToolError> {
        let mut sessions = self.sessions.lock().await;
        let attached = sessions
            .get_mut(&session_id)
            .ok_or_else(|| ToolError::Invalid("computer session is not attached".into()))?;
        if attached.handed_off {
            return Err(ToolError::Invalid(
                "computer session is already handed off".into(),
            ));
        }
        attached.handed_off = true;
        Ok(observation(session_id, attached))
    }

    /// Returns a handed-off target to MCG without capturing it.
    ///
    /// # Errors
    /// Returns an error when the session is absent or was not handed off.
    pub async fn reclaim(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<ComputerUseObservation, ToolError> {
        let mut sessions = self.sessions.lock().await;
        let attached = sessions
            .get_mut(&session_id)
            .ok_or_else(|| ToolError::Invalid("computer session is not attached".into()))?;
        if !attached.handed_off {
            return Err(ToolError::Invalid(
                "computer session has not been handed off".into(),
            ));
        }
        attached.handed_off = false;
        Ok(observation(session_id, attached))
    }

    /// Removes any attached executor state after waiting for in-flight work.
    ///
    /// # Errors
    /// Reserved for executor cleanup failures.
    pub async fn close_if_attached(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<bool, ToolError> {
        Ok(self.sessions.lock().await.remove(&session_id).is_some())
    }
}

fn owned_attached(
    sessions: &HashMap<PlatformSessionId, AttachedComputer>,
    session_id: PlatformSessionId,
) -> Result<AttachedComputer, ToolError> {
    let attached = sessions
        .get(&session_id)
        .ok_or_else(|| ToolError::Invalid("computer session is not attached".into()))?;
    if attached.handed_off {
        return Err(ToolError::Denied(
            "MCG cannot observe or control a computer while it is handed to the local owner".into(),
        ));
    }
    Ok(attached.clone())
}

#[must_use]
pub fn computer_use_runtime_available() -> bool {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return false;
    }
    Screen::all().is_ok_and(|screens| !screens.is_empty())
}

fn resolve_target(target: &ComputerUseTarget) -> Result<AttachedComputer, ToolError> {
    let screens = Screen::all().map_err(|error| {
        ToolError::Unavailable(format!("computer display discovery failed: {error}"))
    })?;
    let screen = target.display_id.map_or_else(
        || screens.iter().find(|screen| screen.display_info.is_primary),
        |id| screens.iter().find(|screen| screen.display_info.id == id),
    );
    let screen = screen.ok_or_else(|| {
        ToolError::Unavailable("the requested computer display is unavailable".into())
    })?;
    let region = target.region.unwrap_or(ComputerUseRegion {
        x: 0,
        y: 0,
        width: screen.display_info.width,
        height: screen.display_info.height,
    });
    validate_region(
        region,
        screen.display_info.width,
        screen.display_info.height,
    )?;
    Ok(AttachedComputer {
        display_id: screen.display_info.id,
        display_x: screen.display_info.x,
        display_y: screen.display_info.y,
        region,
        capabilities: Vec::new(),
        handed_off: false,
    })
}

async fn capture_frame(
    session_id: PlatformSessionId,
    attached: AttachedComputer,
) -> Result<ComputerUseFrame, ToolError> {
    tokio::task::spawn_blocking(move || {
        let screen = Screen::all()
            .map_err(|error| ToolError::Execution(format!("display discovery failed: {error}")))?
            .into_iter()
            .find(|screen| screen.display_info.id == attached.display_id)
            .ok_or_else(|| {
                ToolError::Unavailable("attached computer display disappeared".into())
            })?;
        let image = screen
            .capture_area(
                attached.region.x,
                attached.region.y,
                attached.region.width,
                attached.region.height,
            )
            .map_err(|error| ToolError::Execution(format!("screen capture failed: {error}")))?;
        let mut png = Cursor::new(Vec::new());
        screenshots::image::DynamicImage::ImageRgba8(image)
            .write_to(&mut png, ImageOutputFormat::Png)
            .map_err(|error| ToolError::Execution(format!("screen encoding failed: {error}")))?;
        let png = png.into_inner();
        if png.len() > MAX_FRAME_BYTES {
            return Err(ToolError::Execution(
                "encoded computer frame exceeded the 32 MiB limit".into(),
            ));
        }
        Ok(ComputerUseFrame {
            observation: observation(session_id, &attached),
            sha256: format!("{:x}", Sha256::digest(&png)),
            media_type: "image/png".into(),
            png_base64: STANDARD.encode(png),
            captured_at: Utc::now(),
        })
    })
    .await
    .map_err(|error| ToolError::Execution(format!("computer capture worker failed: {error}")))?
}

fn execute_actions(
    attached: &AttachedComputer,
    actions: &[ComputerUseAction],
) -> Result<(), ToolError> {
    let settings = computer_input_settings();
    let mut enigo = Enigo::new(&settings)
        .map_err(|error| ToolError::Unavailable(format!("computer input unavailable: {error}")))?;
    for action in actions {
        match action {
            ComputerUseAction::Move { x, y } => move_to(&mut enigo, attached, *x, *y)?,
            ComputerUseAction::Click { x, y, button } => {
                move_to(&mut enigo, attached, *x, *y)?;
                enigo
                    .button(map_button(*button), Direction::Click)
                    .map_err(|error| input_error(&error))?;
            }
            ComputerUseAction::Scroll {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                move_to(&mut enigo, attached, *x, *y)?;
                if *delta_x != 0 {
                    enigo
                        .scroll(*delta_x, Axis::Horizontal)
                        .map_err(|error| input_error(&error))?;
                }
                if *delta_y != 0 {
                    enigo
                        .scroll(*delta_y, Axis::Vertical)
                        .map_err(|error| input_error(&error))?;
                }
            }
            ComputerUseAction::TypeText { text } => {
                enigo.text(text).map_err(|error| input_error(&error))?;
            }
            ComputerUseAction::Key { key } => enigo
                .key(map_key(*key), Direction::Click)
                .map_err(|error| input_error(&error))?,
        }
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn computer_input_settings() -> Settings {
    Settings {
        x11_display: std::env::var("DISPLAY").ok(),
        ..Settings::default()
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn computer_input_settings() -> Settings {
    Settings::default()
}

fn move_to(
    enigo: &mut Enigo,
    attached: &AttachedComputer,
    x: u32,
    y: u32,
) -> Result<(), ToolError> {
    validate_point(attached.region, x, y)?;
    let absolute_x = i64::from(attached.display_x) + i64::from(attached.region.x) + i64::from(x);
    let absolute_y = i64::from(attached.display_y) + i64::from(attached.region.y) + i64::from(y);
    enigo
        .move_mouse(
            i32::try_from(absolute_x)
                .map_err(|_| ToolError::Invalid("computer x coordinate overflow".into()))?,
            i32::try_from(absolute_y)
                .map_err(|_| ToolError::Invalid("computer y coordinate overflow".into()))?,
            Coordinate::Abs,
        )
        .map_err(|error| input_error(&error))
}

fn validate_capabilities(capabilities: &[ComputerUseCapability]) -> Result<(), ToolError> {
    if capabilities.is_empty() || capabilities.len() > 6 {
        return Err(ToolError::Invalid(
            "computer sessions require 1 to 6 explicit capabilities".into(),
        ));
    }
    let unique = capabilities
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != capabilities.len() || !unique.contains(&ComputerUseCapability::ScreenRead) {
        return Err(ToolError::Invalid(
            "computer capabilities must be unique and include screen_read".into(),
        ));
    }
    if unique.contains(&ComputerUseCapability::Clipboard)
        || unique.contains(&ComputerUseCapability::WindowList)
        || unique.contains(&ComputerUseCapability::ProcessInspect)
    {
        return Err(ToolError::Unavailable(
            "clipboard, window_list, and process_inspect are not implemented by this executor"
                .into(),
        ));
    }
    Ok(())
}

fn validate_region(region: ComputerUseRegion, width: u32, height: u32) -> Result<(), ToolError> {
    let right = i64::from(region.x) + i64::from(region.width);
    let bottom = i64::from(region.y) + i64::from(region.height);
    if region.x < 0
        || region.y < 0
        || region.width == 0
        || region.height == 0
        || right > i64::from(width)
        || bottom > i64::from(height)
    {
        return Err(ToolError::Invalid(
            "computer region must be non-empty and contained by its display".into(),
        ));
    }
    Ok(())
}

fn validate_actions(actions: &[ComputerUseAction]) -> Result<(), ToolError> {
    if actions.is_empty() || actions.len() > MAX_ACTIONS {
        return Err(ToolError::Invalid(
            "computer input batches require 1 to 32 actions".into(),
        ));
    }
    let text_bytes = actions
        .iter()
        .filter_map(|action| match action {
            ComputerUseAction::TypeText { text } => Some(text.len()),
            _ => None,
        })
        .sum::<usize>();
    if text_bytes > MAX_TEXT_BYTES
        || actions.iter().any(
            |action| matches!(action, ComputerUseAction::TypeText { text } if text.contains('\0')),
        )
    {
        return Err(ToolError::Invalid(
            "computer text must be NUL-free and at most 4096 bytes per batch".into(),
        ));
    }
    let mut focused_inside_region = false;
    for action in actions {
        if matches!(action, ComputerUseAction::Click { .. }) {
            focused_inside_region = true;
        }
        if matches!(
            action,
            ComputerUseAction::TypeText { .. } | ComputerUseAction::Key { .. }
        ) && !focused_inside_region
        {
            return Err(ToolError::Invalid(
                "keyboard input requires an earlier in-region click in the same batch".into(),
            ));
        }
    }
    Ok(())
}

fn validate_action_capabilities(
    attached: &AttachedComputer,
    actions: &[ComputerUseAction],
) -> Result<(), ToolError> {
    for action in actions {
        match action {
            ComputerUseAction::Move { x, y }
            | ComputerUseAction::Click { x, y, .. }
            | ComputerUseAction::Scroll { x, y, .. } => {
                require_capability(attached, ComputerUseCapability::Mouse)?;
                validate_point(attached.region, *x, *y)?;
            }
            ComputerUseAction::TypeText { .. } | ComputerUseAction::Key { .. } => {
                require_capability(attached, ComputerUseCapability::Keyboard)?;
            }
        }
        if let ComputerUseAction::Scroll {
            delta_x, delta_y, ..
        } = action
            && (delta_x.unsigned_abs() > 100 || delta_y.unsigned_abs() > 100)
        {
            return Err(ToolError::Invalid(
                "computer scroll deltas are limited to 100 steps".into(),
            ));
        }
    }
    Ok(())
}

fn validate_point(region: ComputerUseRegion, x: u32, y: u32) -> Result<(), ToolError> {
    if x >= region.width || y >= region.height {
        return Err(ToolError::Invalid(
            "computer input coordinates fall outside the bound region".into(),
        ));
    }
    Ok(())
}

fn require_capability(
    attached: &AttachedComputer,
    required: ComputerUseCapability,
) -> Result<(), ToolError> {
    if attached.capabilities.contains(&required) {
        Ok(())
    } else {
        Err(ToolError::Denied(format!(
            "computer session was not granted {required:?}"
        )))
    }
}

fn observation(
    session_id: PlatformSessionId,
    attached: &AttachedComputer,
) -> ComputerUseObservation {
    ComputerUseObservation {
        session_id,
        display_id: attached.display_id,
        region: attached.region,
        capabilities: attached.capabilities.clone(),
        handed_off: attached.handed_off,
    }
}

const fn map_button(button: ComputerMouseButton) -> Button {
    match button {
        ComputerMouseButton::Left => Button::Left,
        ComputerMouseButton::Middle => Button::Middle,
        ComputerMouseButton::Right => Button::Right,
    }
}

const fn map_key(key: ComputerKey) -> Key {
    match key {
        ComputerKey::Enter => Key::Return,
        ComputerKey::Tab => Key::Tab,
        ComputerKey::Escape => Key::Escape,
        ComputerKey::Backspace => Key::Backspace,
        ComputerKey::Delete => Key::Delete,
        ComputerKey::Space => Key::Space,
        ComputerKey::Home => Key::Home,
        ComputerKey::End => Key::End,
        ComputerKey::PageUp => Key::PageUp,
        ComputerKey::PageDown => Key::PageDown,
        ComputerKey::ArrowUp => Key::UpArrow,
        ComputerKey::ArrowDown => Key::DownArrow,
        ComputerKey::ArrowLeft => Key::LeftArrow,
        ComputerKey::ArrowRight => Key::RightArrow,
    }
}

fn input_error(error: &enigo::InputError) -> ToolError {
    ToolError::Execution(format!("computer input failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attached(capabilities: Vec<ComputerUseCapability>) -> AttachedComputer {
        AttachedComputer {
            display_id: 1,
            display_x: -100,
            display_y: 20,
            region: ComputerUseRegion {
                x: 10,
                y: 15,
                width: 200,
                height: 100,
            },
            capabilities,
            handed_off: false,
        }
    }

    #[test]
    fn grants_require_screen_read_and_reject_unimplemented_power() {
        assert!(validate_capabilities(&[ComputerUseCapability::ScreenRead]).is_ok());
        assert!(validate_capabilities(&[ComputerUseCapability::Mouse]).is_err());
        assert!(
            validate_capabilities(&[
                ComputerUseCapability::ScreenRead,
                ComputerUseCapability::Clipboard,
            ])
            .is_err()
        );
    }

    #[test]
    fn keyboard_requires_same_batch_in_region_focus() {
        assert!(validate_actions(&[ComputerUseAction::TypeText { text: "no".into() }]).is_err());
        assert!(
            validate_actions(&[
                ComputerUseAction::Click {
                    x: 1,
                    y: 1,
                    button: ComputerMouseButton::Left,
                },
                ComputerUseAction::TypeText { text: "yes".into() },
            ])
            .is_ok()
        );
    }

    #[test]
    fn action_validation_enforces_grants_and_region() {
        let attached = attached(vec![
            ComputerUseCapability::ScreenRead,
            ComputerUseCapability::Mouse,
        ]);
        assert!(
            validate_action_capabilities(
                &attached,
                &[ComputerUseAction::Click {
                    x: 199,
                    y: 99,
                    button: ComputerMouseButton::Left,
                }]
            )
            .is_ok()
        );
        assert!(
            validate_action_capabilities(
                &attached,
                &[ComputerUseAction::Click {
                    x: 200,
                    y: 99,
                    button: ComputerMouseButton::Left,
                }]
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn real_capture_is_digest_bound_and_handoff_blocks_reads() {
        if !computer_use_runtime_available() {
            return;
        }
        let manager = ComputerUseManager::new();
        let session_id = PlatformSessionId::new();
        manager
            .attach(
                session_id,
                &ComputerUseTarget::default(),
                &[
                    ComputerUseCapability::ScreenRead,
                    ComputerUseCapability::Mouse,
                ],
            )
            .await
            .expect("attach primary display");
        let frame = manager.capture(session_id).await.expect("capture");
        assert_eq!(frame.media_type, "image/png");
        assert_eq!(frame.sha256.len(), 64);
        assert!(!frame.png_base64.is_empty());
        let stale = manager
            .apply(
                session_id,
                &"0".repeat(64),
                &[ComputerUseAction::Move { x: 0, y: 0 }],
            )
            .await;
        assert!(matches!(stale, Err(ToolError::Denied(_))));
        manager.handoff(session_id).await.expect("handoff");
        assert!(matches!(
            manager.capture(session_id).await,
            Err(ToolError::Denied(_))
        ));
        manager.reclaim(session_id).await.expect("reclaim");
        manager.capture(session_id).await.expect("fresh capture");
        assert!(manager.close_if_attached(session_id).await.expect("close"));
    }
}
