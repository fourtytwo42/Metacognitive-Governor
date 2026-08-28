#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentSchema {
    None,
    OptionalText,
    RequiredText,
    Picker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePolicy {
    Immediate,
    QueueWhenActive,
    RejectWhileActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Always,
    Project,
    FastModel,
    Ide,
    Apps,
    Experimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub arguments: ArgumentSchema,
    pub queue_policy: QueuePolicy,
    pub availability: Availability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandContext {
    pub has_project: bool,
    pub fast_model: bool,
    pub ide: bool,
    pub apps: bool,
    pub experimental: bool,
}

impl SlashCommandSpec {
    #[must_use]
    pub fn is_available(self, context: CommandContext) -> bool {
        match self.availability {
            Availability::Always => true,
            Availability::Project => context.has_project,
            Availability::FastModel => context.fast_model,
            Availability::Ide => context.ide,
            Availability::Apps => context.apps,
            Availability::Experimental => context.experimental,
        }
    }

    #[must_use]
    pub fn matches(self, name: &str) -> bool {
        self.name == name || self.aliases.contains(&name)
    }
}

macro_rules! command {
    ($name:literal, $description:literal, $arguments:ident, $queue:ident, $availability:ident) => {
        SlashCommandSpec {
            name: $name,
            aliases: &[],
            description: $description,
            arguments: ArgumentSchema::$arguments,
            queue_policy: QueuePolicy::$queue,
            availability: Availability::$availability,
        }
    };
    ($name:literal, [$($alias:literal),+], $description:literal, $arguments:ident, $queue:ident, $availability:ident) => {
        SlashCommandSpec {
            name: $name,
            aliases: &[$($alias),+],
            description: $description,
            arguments: ArgumentSchema::$arguments,
            queue_policy: QueuePolicy::$queue,
            availability: Availability::$availability,
        }
    };
}

pub const SLASH_COMMANDS: &[SlashCommandSpec] = &[
    command!(
        "new",
        "Start a new conversation",
        OptionalText,
        Immediate,
        Always
    ),
    command!(
        "clear",
        "Clear the display and start a new conversation",
        None,
        Immediate,
        Always
    ),
    command!(
        "resume",
        "Resume a previous conversation",
        Picker,
        Immediate,
        Always
    ),
    command!(
        "rename",
        "Rename the current conversation",
        RequiredText,
        Immediate,
        Always
    ),
    command!(
        "fork",
        "Fork from a selected user turn",
        Picker,
        Immediate,
        Always
    ),
    command!(
        "side",
        ["btw"],
        "Open an ephemeral side conversation",
        OptionalText,
        Immediate,
        Always
    ),
    command!(
        "archive",
        "Archive the current conversation",
        None,
        RejectWhileActive,
        Always
    ),
    command!(
        "delete",
        "Delete the current conversation after confirmation",
        None,
        RejectWhileActive,
        Experimental
    ),
    command!(
        "exit",
        ["quit"],
        "Detach from durable work and close MetaCog",
        None,
        Immediate,
        Always
    ),
    command!(
        "review",
        "Run a native read-only working-tree review",
        OptionalText,
        QueueWhenActive,
        Project
    ),
    command!(
        "diff",
        "Show working-tree changes",
        None,
        Immediate,
        Project
    ),
    command!(
        "compact",
        "Compact the current Codex context",
        None,
        QueueWhenActive,
        Always
    ),
    command!(
        "copy",
        "Copy the latest completed response",
        None,
        Immediate,
        Always
    ),
    command!(
        "status",
        "Show project, model, context, usage, and limits",
        None,
        Immediate,
        Always
    ),
    command!(
        "usage",
        "Show current account usage and limits",
        None,
        Immediate,
        Always
    ),
    command!(
        "raw",
        "Toggle raw event display",
        None,
        Immediate,
        Experimental
    ),
    command!(
        "ps",
        "Show commands owned by this conversation",
        None,
        Immediate,
        Experimental
    ),
    command!(
        "stop",
        "Stop a command owned by this conversation",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "model",
        "Choose model, effort, and speed",
        Picker,
        Immediate,
        Always
    ),
    command!(
        "fast",
        "Toggle the model's supported fast service tier",
        None,
        Immediate,
        FastModel
    ),
    command!(
        "personality",
        "Choose friendly, pragmatic, or none",
        Picker,
        Immediate,
        Always
    ),
    command!(
        "permissions",
        "Choose the conversation approval policy",
        Picker,
        Immediate,
        Always
    ),
    command!(
        "approve",
        "Approve the exact recent auto-review denial",
        None,
        Immediate,
        Always
    ),
    command!(
        "plan",
        "Enter Plan mode",
        OptionalText,
        QueueWhenActive,
        Always
    ),
    command!(
        "goal",
        "Set or manage a persistent goal",
        OptionalText,
        QueueWhenActive,
        Always
    ),
    command!(
        "mention",
        "Open the file and folder picker",
        Picker,
        Immediate,
        Always
    ),
    command!("skills", "List available skills", None, Immediate, Always),
    command!("apps", "List enabled apps", None, Immediate, Apps),
    command!(
        "plugins",
        "List enabled MetaCog plugins",
        None,
        Immediate,
        Always
    ),
    command!("mcp", "List connected MCP servers", None, Immediate, Always),
    command!(
        "memories",
        "Inspect memory configuration",
        None,
        Immediate,
        Always
    ),
    command!(
        "agent",
        "Select an agent thread",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "subagents",
        "Inspect subagent threads",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "experimental",
        "Configure discovered experimental features",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "keymap",
        "Choose terminal key bindings",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "vim",
        "Toggle Vim editing mode",
        None,
        Immediate,
        Experimental
    ),
    command!(
        "statusline",
        "Configure status-line fields",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "title",
        "Configure terminal title fields",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "theme",
        "Choose a terminal theme",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "pets",
        ["pet"],
        "Choose a terminal pet",
        Picker,
        Immediate,
        Experimental
    ),
    command!(
        "debug-config",
        "Show effective CLI configuration",
        None,
        Immediate,
        Experimental
    ),
    command!("ide", "Connect the detected IDE", Picker, Immediate, Ide),
    command!(
        "init",
        "Create or improve project AGENTS.md",
        None,
        QueueWhenActive,
        Project
    ),
    command!(
        "import",
        "Import settings or a conversation",
        RequiredText,
        Immediate,
        Experimental
    ),
    command!("hooks", "Inspect configured hooks", None, Immediate, Always),
    command!(
        "logout",
        "Disconnect MetaCog from Codex after confirmation",
        None,
        Immediate,
        Experimental
    ),
    command!(
        "app",
        "Open this conversation in the MetaCog desktop app",
        None,
        Immediate,
        Experimental
    ),
    command!(
        "feedback",
        "Show the feedback workflow",
        None,
        Immediate,
        Always
    ),
    command!(
        "setup-default-sandbox",
        "Configure the Windows default sandbox",
        None,
        Immediate,
        Experimental
    ),
    command!(
        "sandbox-add-read-dir",
        "Add a Windows sandbox read directory",
        RequiredText,
        Immediate,
        Experimental
    ),
    command!(
        "mcg",
        "MetaCog-specific project and diagnostic operations",
        OptionalText,
        Immediate,
        Always
    ),
    command!(
        "help",
        "Show commands and keyboard controls",
        None,
        Immediate,
        Always
    ),
];

#[must_use]
pub fn command_named(name: &str, context: CommandContext) -> Option<&'static SlashCommandSpec> {
    SLASH_COMMANDS
        .iter()
        .find(|command| command.matches(name) && command.is_available(context))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve_to_the_canonical_command() {
        let context = CommandContext::default();
        assert_eq!(
            command_named("quit", context).map(|value| value.name),
            Some("exit")
        );
        assert_eq!(
            command_named("btw", context).map(|value| value.name),
            Some("side")
        );
    }

    #[test]
    fn unavailable_commands_are_hidden_instead_of_returning_boundaries() {
        let context = CommandContext::default();
        assert!(command_named("review", context).is_none());
        assert!(command_named("fast", context).is_none());
        assert!(command_named("apps", context).is_none());
        assert!(
            SLASH_COMMANDS
                .iter()
                .any(|command| command.name == "review")
        );
    }

    #[test]
    fn shared_command_names_are_unique() {
        let mut names = std::collections::BTreeSet::new();
        for command in SLASH_COMMANDS {
            assert!(names.insert(command.name), "duplicate /{}", command.name);
        }
    }
}
