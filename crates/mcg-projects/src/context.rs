use std::{collections::BTreeSet, path::Path};

use crate::{
    ContextAssembly, ContextCategory, ContextItem, InstructionResolution, InstructionResolver,
    ProjectModel, WorldNodeKind,
};

pub struct ContextAssembler;

impl ContextAssembler {
    #[must_use]
    pub fn assemble(mut items: Vec<ContextItem>, token_budget: u32) -> ContextAssembly {
        for item in &mut items {
            item.estimated_tokens = estimate_tokens(&item.content);
        }
        items.sort_by(|left, right| {
            category_rank(left.category)
                .cmp(&category_rank(right.category))
                .then_with(|| right.priority.cmp(&left.priority))
                .then_with(|| left.source.cmp(&right.source))
                .then_with(|| left.content.cmp(&right.content))
        });

        let original_len = items.len();
        let mut selected = Vec::new();
        let mut used = 0_u32;
        for mut item in items {
            let remaining = token_budget.saturating_sub(used);
            if item.estimated_tokens <= remaining {
                used = used.saturating_add(item.estimated_tokens);
                selected.push(item);
            } else if is_protected(item.category) && remaining >= 8 {
                item.content = truncate_to_tokens(&item.content, remaining);
                item.estimated_tokens = estimate_tokens(&item.content);
                used = used.saturating_add(item.estimated_tokens);
                selected.push(item);
            }
            if used >= token_budget {
                break;
            }
        }
        let omitted_items =
            u32::try_from(original_len.saturating_sub(selected.len())).unwrap_or(u32::MAX);
        ContextAssembly {
            items: selected,
            omitted_items,
            estimated_tokens: used,
            compression_round: u16::from(omitted_items > 0),
        }
    }

    #[must_use]
    pub fn from_project(model: &ProjectModel) -> Vec<ContextItem> {
        Self::from_project_for_target(model, "")
    }

    #[must_use]
    pub fn from_project_for_target(
        model: &ProjectModel,
        relative_target: &str,
    ) -> Vec<ContextItem> {
        let instructions =
            InstructionResolver::for_target(&model.instructions.documents, relative_target);
        let documents = InstructionResolver::applicable_documents(
            &model.instructions.documents,
            relative_target,
        );
        Self::from_project_resolution(model, &instructions, &documents)
    }

    fn from_project_resolution(
        model: &ProjectModel,
        instructions: &InstructionResolution,
        documents: &[&crate::InstructionDocument],
    ) -> Vec<ContextItem> {
        let mut items = Vec::new();
        for document in documents {
            items.push(ContextItem {
                category: ContextCategory::Instruction,
                content: document.content.clone(),
                source: document.path.clone(),
                priority: document.priority,
                estimated_tokens: 0,
            });
        }
        for instruction in &instructions.effective {
            items.push(ContextItem {
                category: ContextCategory::Instruction,
                content: format!("{}: {}", instruction.key, instruction.value),
                source: instruction.source_path.clone(),
                priority: instruction.priority,
                estimated_tokens: 0,
            });
        }
        for conflict in &instructions.conflicts {
            items.push(ContextItem {
                category: ContextCategory::OpenQuestion,
                content: format!(
                    "Instruction conflict for {}: {} ({:?})",
                    conflict.key,
                    conflict.values.join(", "),
                    conflict.resolution
                ),
                source: conflict.source_paths.join(", "),
                priority: 1_000,
                estimated_tokens: 0,
            });
        }
        for command in &model.commands {
            items.push(ContextItem {
                category: ContextCategory::ProjectFact,
                content: format!("{} command: {}", command.name, command.command),
                source: command.source.clone(),
                priority: 500,
                estimated_tokens: 0,
            });
        }
        for node in &model.world.nodes {
            if matches!(
                node.kind,
                WorldNodeKind::Repository | WorldNodeKind::Command
            ) {
                continue;
            }
            items.push(ContextItem {
                category: ContextCategory::ProjectFact,
                content: format!("World {}: {}", world_kind_label(node.kind), node.label),
                source: node.source.clone(),
                priority: world_node_priority(node.kind),
                estimated_tokens: 0,
            });
        }
        for relationship in &model.world.relationships {
            let from = world_label(model, &relationship.from);
            let to = world_label(model, &relationship.to);
            items.push(ContextItem {
                category: ContextCategory::ProjectFact,
                content: format!(
                    "World relationship: {from} -> {} -> {to}",
                    relationship.relation.replace('_', " ")
                ),
                source: relationship.source.clone(),
                // Relationships carry more planning value than isolated inventory
                // nodes because they explain how two known components interact.
                priority: world_relationship_priority(&relationship.relation),
                estimated_tokens: 0,
            });
        }
        items
    }
}

fn world_label<'a>(model: &'a ProjectModel, id: &'a str) -> &'a str {
    model
        .world
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or(id, |node| node.label.as_str())
}

const fn world_kind_label(kind: WorldNodeKind) -> &'static str {
    match kind {
        WorldNodeKind::Service => "service",
        WorldNodeKind::Database => "database",
        WorldNodeKind::Domain => "domain",
        WorldNodeKind::Api => "API",
        WorldNodeKind::Deployment => "deployment",
        WorldNodeKind::Dependency => "dependency",
        WorldNodeKind::Repository => "repository",
        WorldNodeKind::Runtime => "runtime",
        WorldNodeKind::Command => "command",
    }
}

const fn world_node_priority(kind: WorldNodeKind) -> u16 {
    match kind {
        WorldNodeKind::Service | WorldNodeKind::Database | WorldNodeKind::Api => 700,
        WorldNodeKind::Domain | WorldNodeKind::Deployment => 650,
        WorldNodeKind::Runtime => 600,
        WorldNodeKind::Dependency => 400,
        WorldNodeKind::Repository | WorldNodeKind::Command => 300,
    }
}

fn world_relationship_priority(relation: &str) -> u16 {
    match relation {
        "uses_database" | "depends_on" => 900,
        "served_at" | "deploys_to" | "exposes" => 850,
        "runs_on" | "uses_runtime" | "uses_image" => 800,
        _ => 750,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInvalidation {
    pub changed_inputs: Vec<String>,
    pub invalidate_instructions: bool,
    pub invalidate_commands: bool,
    pub invalidate_world_model: bool,
}

impl SnapshotInvalidation {
    #[must_use]
    pub fn between(previous: &ProjectModel, current: &ProjectModel) -> Self {
        let paths = previous
            .watched_inputs
            .keys()
            .chain(current.watched_inputs.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let changed_inputs = paths
            .into_iter()
            .filter(|path| previous.watched_inputs.get(path) != current.watched_inputs.get(path))
            .collect::<Vec<_>>();
        let invalidate_instructions = changed_inputs.iter().any(|path| {
            Path::new(path).extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("mdc")
            }) || path.contains("/rules/")
        });
        let invalidate_commands = changed_inputs.iter().any(|path| {
            matches!(
                path.as_str(),
                "Cargo.toml" | "package.json" | "Makefile" | "pyproject.toml" | "go.mod"
            )
        });
        let invalidate_world_model = invalidate_commands
            || changed_inputs.iter().any(|path| {
                path.contains("compose")
                    || path == "Dockerfile"
                    || matches!(
                        path.rsplit('/').next().unwrap_or(path),
                        "openapi.json"
                            | "openapi.yaml"
                            | "openapi.yml"
                            | "swagger.json"
                            | "swagger.yaml"
                            | "swagger.yml"
                            | "vercel.json"
                            | "netlify.toml"
                            | "fly.toml"
                            | "render.yaml"
                            | "render.yml"
                            | "wrangler.toml"
                            | "railway.json"
                            | "requirements.txt"
                    )
                    || path.starts_with(".github/workflows/")
            });
        Self {
            changed_inputs,
            invalidate_instructions,
            invalidate_commands,
            invalidate_world_model,
        }
    }
}

fn estimate_tokens(content: &str) -> u32 {
    let characters = u32::try_from(content.chars().count()).unwrap_or(u32::MAX);
    characters.saturating_add(3) / 4
}

fn truncate_to_tokens(content: &str, tokens: u32) -> String {
    let character_limit = usize::try_from(tokens.saturating_mul(4)).unwrap_or(usize::MAX);
    let mut truncated = content.chars().take(character_limit).collect::<String>();
    if truncated.chars().count() < content.chars().count() {
        let suffix = "…";
        while estimate_tokens(&(truncated.clone() + suffix)) > tokens {
            truncated.pop();
        }
        truncated.push_str(suffix);
    }
    truncated
}

const fn is_protected(category: ContextCategory) -> bool {
    matches!(
        category,
        ContextCategory::Goal
            | ContextCategory::Constraint
            | ContextCategory::Decision
            | ContextCategory::OpenQuestion
            | ContextCategory::Evidence
            | ContextCategory::Plan
    )
}

const fn category_rank(category: ContextCategory) -> u8 {
    match category {
        ContextCategory::Goal => 0,
        ContextCategory::Constraint => 1,
        ContextCategory::Decision => 2,
        ContextCategory::OpenQuestion => 3,
        ContextCategory::Evidence => 4,
        ContextCategory::Plan => 5,
        ContextCategory::Instruction => 6,
        ContextCategory::Memory => 7,
        ContextCategory::ProjectFact => 8,
        ContextCategory::Transcript => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::ProjectId;
    use tempfile::tempdir;

    fn item(category: ContextCategory, content: &str) -> ContextItem {
        ContextItem {
            category,
            content: content.into(),
            source: "fixture".into(),
            priority: 1,
            estimated_tokens: 0,
        }
    }

    #[test]
    fn compression_preserves_goal_before_old_transcript() {
        let assembly = ContextAssembler::assemble(
            vec![
                item(ContextCategory::Transcript, &"old ".repeat(200)),
                item(ContextCategory::Goal, "Ship a verified project"),
                item(
                    ContextCategory::Constraint,
                    "Never publish without approval",
                ),
            ],
            24,
        );
        assert!(assembly.estimated_tokens <= 24);
        assert_eq!(assembly.compression_round, 1);
        assert_eq!(assembly.items[0].category, ContextCategory::Goal);
        assert!(
            assembly
                .items
                .iter()
                .any(|item| item.category == ContextCategory::Constraint)
        );
        assert!(
            assembly
                .items
                .iter()
                .all(|item| item.category != ContextCategory::Transcript)
        );
    }

    #[test]
    fn bounded_context_uses_readable_topology_with_manifest_provenance() {
        let root = tempdir().expect("project");
        std::fs::write(
            root.path().join("compose.yaml"),
            "services:\n  api:\n    image: example/api:1\n    depends_on:\n      - db\n  db:\n    image: postgres:17\n",
        )
        .expect("compose");
        let model =
            crate::ProjectDiscoverer::discover(root.path(), ProjectId::new()).expect("discovery");
        let items = ContextAssembler::from_project(&model);
        assert!(items.iter().any(|item| {
            item.content == "World database: db (PostgreSQL)"
                && item.source == "compose.yaml#services.db.image"
        }));
        assert!(items.iter().any(|item| {
            item.content == "World relationship: api -> uses database -> db (PostgreSQL)"
                && item.source == "compose.yaml#services.api.depends_on"
        }));
        assert!(model.world.nodes.iter().all(|node| {
            node.id == "repository" || items.iter().all(|item| !item.content.contains(&node.id))
        }));

        let assembly = ContextAssembler::assemble(items, 48);
        assert!(assembly.estimated_tokens <= 48);
        assert!(
            assembly
                .items
                .iter()
                .any(|item| item.content.contains("api -> uses database -> db"))
        );
    }
}
