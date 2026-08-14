use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use mcg_domain::{ProjectId, ProjectSnapshotId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ConflictResolution, EffectiveInstruction, InstructionConflict, InstructionDocument,
    InstructionKind, InstructionResolution, PROJECT_MODEL_SCHEMA_VERSION, ProjectCommand,
    ProjectModel, ProjectWorldModel, RecentCommit, RepositoryModel, SnapshotInvalidation,
    WorldNode, WorldNodeKind, WorldRelationship,
};

const MAX_FILES: usize = 20_000;
const MAX_DEPTH: usize = 12;
const MAX_INSTRUCTION_BYTES: u64 = 512 * 1024;

#[derive(Debug, Error)]
pub enum ProjectDiscoveryError {
    #[error("project path does not exist or is not a directory: {0}")]
    InvalidRoot(String),
    #[error("project discovery I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("project manifest is invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct ProjectDiscoverer;

pub struct InstructionResolver;

pub struct ProjectWatcher {
    current: ProjectModel,
}

impl ProjectWatcher {
    /// Starts a polling watch from a freshly discovered baseline.
    ///
    /// # Errors
    ///
    /// Returns project discovery errors for inaccessible/invalid roots.
    pub fn start(root: &Path, project_id: ProjectId) -> Result<Self, ProjectDiscoveryError> {
        Ok(Self {
            current: ProjectDiscoverer::discover(root, project_id)?,
        })
    }

    #[must_use]
    pub const fn current(&self) -> &ProjectModel {
        &self.current
    }

    /// Re-scans bounded watched inputs and returns only meaningful changes.
    ///
    /// # Errors
    ///
    /// Returns project discovery errors without replacing the last valid model.
    pub fn poll(
        &mut self,
    ) -> Result<Option<(ProjectModel, SnapshotInvalidation)>, ProjectDiscoveryError> {
        let next =
            ProjectDiscoverer::discover(Path::new(&self.current.root), self.current.project_id)?;
        if next.fingerprint == self.current.fingerprint {
            return Ok(None);
        }
        let invalidation = SnapshotInvalidation::between(&self.current, &next);
        self.current = next.clone();
        Ok(Some((next, invalidation)))
    }
}

impl InstructionResolver {
    #[must_use]
    pub fn applicable_documents<'a>(
        documents: &'a [InstructionDocument],
        relative_target: &str,
    ) -> Vec<&'a InstructionDocument> {
        let target = relative_target.replace('\\', "/");
        documents
            .iter()
            .filter(|document| scope_applies(&document.scope, &target))
            .collect()
    }

    /// Resolves only documents applicable to a project-relative target path while
    /// retaining the full discovered document inventory for inspection.
    #[must_use]
    pub fn for_target(
        documents: &[InstructionDocument],
        relative_target: &str,
    ) -> InstructionResolution {
        let applicable = Self::applicable_documents(documents, relative_target)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = package_manager_candidates(&applicable);
        let mut effective = Vec::new();
        let mut conflicts = Vec::new();
        if let Some(first) = candidates.first() {
            effective.push(EffectiveInstruction {
                key: "package_manager".into(),
                value: first.0.clone(),
                source_path: first.1.path.clone(),
                priority: first.1.priority,
            });
            let distinct = candidates
                .iter()
                .map(|(value, _)| value.clone())
                .collect::<BTreeSet<_>>();
            if distinct.len() > 1 {
                let top_values = candidates
                    .iter()
                    .filter(|(_, document)| document.priority == first.1.priority)
                    .map(|(value, _)| value)
                    .collect::<BTreeSet<_>>();
                let resolution = if top_values.len() > 1 {
                    ConflictResolution::Unresolved
                } else if candidates
                    .iter()
                    .any(|(_, document)| document.priority < first.1.priority)
                    && candidates
                        .iter()
                        .any(|(_, document)| document.scope != first.1.scope)
                {
                    ConflictResolution::MoreSpecific
                } else {
                    ConflictResolution::HigherPrecedence
                };
                conflicts.push(InstructionConflict {
                    key: "package_manager".into(),
                    values: distinct.into_iter().collect(),
                    source_paths: candidates
                        .iter()
                        .map(|(_, document)| document.path.clone())
                        .collect(),
                    resolution,
                });
            }
        }
        InstructionResolution {
            documents: documents.to_vec(),
            effective,
            conflicts,
        }
    }
}

impl ProjectDiscoverer {
    /// Builds a stable, bounded project snapshot without following symlinks.
    ///
    /// # Errors
    ///
    /// Returns an error when the root cannot be resolved or a discovered manifest
    /// required for command extraction contains invalid JSON.
    pub fn discover(
        requested_root: &Path,
        project_id: ProjectId,
    ) -> Result<ProjectModel, ProjectDiscoveryError> {
        if !requested_root.is_dir() {
            return Err(ProjectDiscoveryError::InvalidRoot(
                requested_root.display().to_string(),
            ));
        }
        let requested_root = requested_root.canonicalize()?;
        let root = find_repository_root(&requested_root).unwrap_or(requested_root);
        let files = collect_files(&root)?;
        let instructions = resolve_instructions(&root, &files)?;
        let commands = discover_commands(&root, &files)?;
        let repository = discover_repository(&root);
        let mut watched_inputs = watched_inputs(&root, &files)?;
        if let Some(head) = &repository.head {
            watched_inputs.insert("@git/head".into(), sha256(head.as_bytes()));
        }
        watched_inputs.insert(
            "@git/dirty".into(),
            sha256(repository.dirty.to_string().as_bytes()),
        );
        let fingerprint = fingerprint(&watched_inputs);
        let languages = discover_languages(&files);
        let build_systems = discover_build_systems(&files);
        let test_systems = discover_test_systems(&root, &files)?;
        let package_managers = discover_package_managers(&files);
        let services = discover_services(&files);
        let ci_systems = discover_ci(&files);
        let important_directories = discover_important_directories(&root);
        let existing_memory = files
            .iter()
            .find(|path| relative(&root, path) == "MEMORY.md")
            .map(|_| "MEMORY.md".to_owned());
        let dependencies = discover_dependencies(&root, &files)?;
        let world = world_model(&commands, &services, &repository, &dependencies);

        Ok(ProjectModel {
            schema_version: PROJECT_MODEL_SCHEMA_VERSION,
            id: ProjectSnapshotId::new(),
            project_id,
            root: root.to_string_lossy().into_owned(),
            fingerprint,
            observed_at: Utc::now(),
            repository,
            languages,
            build_systems,
            test_systems,
            package_managers,
            services,
            ci_systems,
            important_directories,
            commands,
            instructions,
            existing_memory,
            world,
            watched_inputs,
        })
    }
}

fn find_repository_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf)
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH || files.len() >= MAX_FILES {
            continue;
        }
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                if !ignored_directory(&entry.file_name().to_string_lossy()) {
                    pending.push((path, depth + 1));
                }
            } else if file_type.is_file() && files.len() < MAX_FILES {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn ignored_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".venv"
            | "venv"
            | "__pycache__"
    )
}

fn resolve_instructions(
    root: &Path,
    files: &[PathBuf],
) -> Result<InstructionResolution, std::io::Error> {
    let mut documents = Vec::new();
    for path in files {
        let relative_path = relative(root, path);
        let Some((kind, base_priority)) = instruction_kind(&relative_path) else {
            continue;
        };
        if path.metadata()?.len() > MAX_INSTRUCTION_BYTES {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let scope_path = if relative_path.starts_with(".claude/rules/")
            || relative_path.starts_with(".cursor/rules/")
        {
            root
        } else {
            path.parent().unwrap_or(root)
        };
        let scope = relative(root, scope_path);
        let specificity = u16::try_from(scope.matches('/').count()).unwrap_or(u16::MAX);
        documents.push(InstructionDocument {
            path: relative_path,
            kind,
            scope,
            priority: base_priority.saturating_add(specificity),
            sha256: sha256(content.as_bytes()),
            content,
        });
    }
    documents.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(InstructionResolver::for_target(&documents, ""))
}

fn scope_applies(scope: &str, target: &str) -> bool {
    scope.is_empty() || target == scope || target.starts_with(&format!("{scope}/"))
}

fn instruction_kind(path: &str) -> Option<(InstructionKind, u16)> {
    let name = path.rsplit('/').next()?;
    match name {
        "AGENTS.override.md" => Some((InstructionKind::AgentsOverride, 900)),
        "AGENTS.md" => Some((InstructionKind::Agents, 800)),
        "CLAUDE.local.md" => Some((InstructionKind::ClaudeLocal, 700)),
        "CLAUDE.md" => Some((InstructionKind::Claude, 600)),
        "README.md" => Some((InstructionKind::Readme, 200)),
        "CONTRIBUTING.md" => Some((InstructionKind::Contributing, 300)),
        _ if path.starts_with(".claude/rules/") && has_extension(path, "md") => {
            Some((InstructionKind::ClaudeRule, 500))
        }
        _ if path.starts_with(".cursor/rules/") && has_extension(path, "mdc") => {
            Some((InstructionKind::CursorRule, 400))
        }
        _ => None,
    }
}

fn has_extension(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn package_manager_candidates(
    documents: &[InstructionDocument],
) -> Vec<(String, &InstructionDocument)> {
    let mut candidates = Vec::new();
    for document in documents {
        let content = document.content.to_ascii_lowercase();
        for manager in ["pnpm", "npm", "yarn", "bun"] {
            let patterns = [
                format!("use {manager}"),
                format!("using {manager}"),
                format!("`{manager} "),
                format!("manager: {manager}"),
            ];
            if patterns.iter().any(|pattern| content.contains(pattern)) {
                candidates.push((manager.to_owned(), document));
                break;
            }
        }
    }
    candidates.sort_by(|left, right| right.1.priority.cmp(&left.1.priority));
    candidates
}

fn discover_commands(
    root: &Path,
    files: &[PathBuf],
) -> Result<Vec<ProjectCommand>, ProjectDiscoveryError> {
    let mut commands = Vec::new();
    if files
        .iter()
        .any(|path| relative(root, path) == "Cargo.toml")
    {
        commands.extend([
            command("build", "cargo build", "Cargo.toml"),
            command("test", "cargo test", "Cargo.toml"),
            command("check", "cargo clippy --all-targets", "Cargo.toml"),
        ]);
    }
    if let Some(package_json) = files
        .iter()
        .find(|path| relative(root, path) == "package.json")
    {
        let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
        if let Some(scripts) = value.get("scripts").and_then(serde_json::Value::as_object) {
            let runner = preferred_js_runner(files);
            for name in ["build", "test", "check", "lint", "dev", "start", "test:e2e"] {
                if scripts.contains_key(name) {
                    commands.push(command(
                        name,
                        &format!("{runner} run {name}"),
                        "package.json",
                    ));
                }
            }
        }
    }
    if files.iter().any(|path| relative(root, path) == "Makefile") {
        commands.push(command("build", "make", "Makefile"));
        commands.push(command("test", "make test", "Makefile"));
    }
    commands.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.command.cmp(&right.command))
    });
    commands.dedup_by(|left, right| left.name == right.name && left.command == right.command);
    Ok(commands)
}

fn command(name: &str, value: &str, source: &str) -> ProjectCommand {
    ProjectCommand {
        name: name.into(),
        command: value.into(),
        source: source.into(),
        confidence_basis: "declared project manifest".into(),
    }
}

fn preferred_js_runner(files: &[PathBuf]) -> &'static str {
    if files.iter().any(|path| path.ends_with("pnpm-lock.yaml")) {
        "pnpm"
    } else if files.iter().any(|path| path.ends_with("yarn.lock")) {
        "yarn"
    } else if files
        .iter()
        .any(|path| path.ends_with("bun.lockb") || path.ends_with("bun.lock"))
    {
        "bun"
    } else {
        "npm"
    }
}

fn discover_repository(root: &Path) -> RepositoryModel {
    if !root.join(".git").exists() {
        return RepositoryModel::default();
    }
    let branch = git(root, &["branch", "--show-current"]);
    let head = git(root, &["rev-parse", "HEAD"]);
    let dirty = git(root, &["status", "--porcelain"]).is_some_and(|value| !value.is_empty());
    let recent_commits = git(root, &["log", "-5", "--pretty=format:%H%x09%s"])
        .map(|output| {
            output
                .lines()
                .filter_map(|line| line.split_once('\t'))
                .map(|(id, summary)| RecentCommit {
                    id: id.to_owned(),
                    summary: summary.to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    RepositoryModel {
        is_git: true,
        branch,
        head,
        dirty,
        recent_commits,
    }
}

fn git(root: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn watched_inputs(
    root: &Path,
    files: &[PathBuf],
) -> Result<BTreeMap<String, String>, std::io::Error> {
    let mut inputs = BTreeMap::new();
    for path in files {
        let relative_path = relative(root, path);
        if instruction_kind(&relative_path).is_some() || is_manifest(&relative_path) {
            inputs.insert(relative_path, sha256(&fs::read(path)?));
        }
    }
    Ok(inputs)
}

fn is_manifest(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lock"
            | "bun.lockb"
            | "pyproject.toml"
            | "requirements.txt"
            | "go.mod"
            | "Dockerfile"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | "compose.yml"
            | "compose.yaml"
            | "Makefile"
    ) || path.starts_with(".github/workflows/")
}

fn fingerprint(inputs: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (path, hash) in inputs {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(hash.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn discover_languages(files: &[PathBuf]) -> Vec<String> {
    let mut languages = BTreeSet::new();
    for path in files {
        let language = match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => Some("Rust"),
            Some("ts" | "tsx") => Some("TypeScript"),
            Some("js" | "jsx" | "mjs" | "cjs") => Some("JavaScript"),
            Some("py") => Some("Python"),
            Some("go") => Some("Go"),
            Some("java") => Some("Java"),
            Some("kt" | "kts") => Some("Kotlin"),
            Some("cs") => Some("C#"),
            Some("cpp" | "cc" | "cxx" | "hpp") => Some("C++"),
            Some("c" | "h") => Some("C"),
            _ => None,
        };
        if let Some(language) = language {
            languages.insert(language.to_owned());
        }
    }
    languages.into_iter().collect()
}

fn discover_build_systems(files: &[PathBuf]) -> Vec<String> {
    named_presence(
        files,
        &[
            ("Cargo.toml", "Cargo"),
            ("package.json", "Node package scripts"),
            ("Makefile", "Make"),
            ("CMakeLists.txt", "CMake"),
            ("pyproject.toml", "Python pyproject"),
            ("go.mod", "Go modules"),
        ],
    )
}

fn discover_test_systems(
    root: &Path,
    files: &[PathBuf],
) -> Result<Vec<String>, ProjectDiscoveryError> {
    let mut systems = BTreeSet::new();
    if files
        .iter()
        .any(|path| relative(root, path) == "Cargo.toml")
    {
        systems.insert("cargo test".to_owned());
    }
    if let Some(package_json) = files
        .iter()
        .find(|path| relative(root, path) == "package.json")
    {
        let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
        let all = value.to_string().to_ascii_lowercase();
        for (needle, name) in [
            ("vitest", "Vitest"),
            ("jest", "Jest"),
            ("playwright", "Playwright"),
            ("cypress", "Cypress"),
        ] {
            if all.contains(needle) {
                systems.insert(name.to_owned());
            }
        }
    }
    if files.iter().any(|path| path.ends_with("pytest.ini")) {
        systems.insert("pytest".to_owned());
    }
    Ok(systems.into_iter().collect())
}

fn discover_package_managers(files: &[PathBuf]) -> Vec<String> {
    named_presence(
        files,
        &[
            ("Cargo.lock", "Cargo"),
            ("package-lock.json", "npm"),
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "Yarn"),
            ("bun.lock", "Bun"),
            ("go.sum", "Go modules"),
        ],
    )
}

fn discover_services(files: &[PathBuf]) -> Vec<String> {
    named_presence(
        files,
        &[
            ("Dockerfile", "Docker service"),
            ("docker-compose.yml", "Docker Compose"),
            ("docker-compose.yaml", "Docker Compose"),
            ("compose.yml", "Docker Compose"),
            ("compose.yaml", "Docker Compose"),
        ],
    )
}

fn discover_dependencies(
    root: &Path,
    files: &[PathBuf],
) -> Result<Vec<(String, String)>, ProjectDiscoveryError> {
    let Some(package_json) = files
        .iter()
        .find(|path| relative(root, path) == "package.json")
    else {
        return Ok(Vec::new());
    };
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
    let mut dependencies = BTreeSet::new();
    for section in ["dependencies", "devDependencies"] {
        if let Some(values) = value.get(section).and_then(serde_json::Value::as_object) {
            dependencies.extend(
                values
                    .keys()
                    .take(500)
                    .map(|name| (name.clone(), format!("package.json#{section}"))),
            );
        }
    }
    Ok(dependencies.into_iter().collect())
}

fn discover_ci(files: &[PathBuf]) -> Vec<String> {
    let mut systems = BTreeSet::new();
    for path in files {
        let normalized = path.to_string_lossy().replace('\\', "/");
        if normalized.contains("/.github/workflows/") {
            systems.insert("GitHub Actions".to_owned());
        }
        if normalized.ends_with("/.gitlab-ci.yml") {
            systems.insert("GitLab CI".to_owned());
        }
    }
    systems.into_iter().collect()
}

fn named_presence(files: &[PathBuf], names: &[(&str, &str)]) -> Vec<String> {
    let mut found = BTreeSet::new();
    for path in files {
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            for (candidate, label) in names {
                if name == *candidate {
                    found.insert((*label).to_owned());
                }
            }
        }
    }
    found.into_iter().collect()
}

fn discover_important_directories(root: &Path) -> Vec<String> {
    [
        "src",
        "app",
        "apps",
        "crates",
        "packages",
        "tests",
        "docs",
        "scripts",
        "infra",
        "migrations",
    ]
    .into_iter()
    .filter(|directory| root.join(directory).is_dir())
    .map(str::to_owned)
    .collect()
}

fn world_model(
    commands: &[ProjectCommand],
    services: &[String],
    repository: &RepositoryModel,
    dependencies: &[(String, String)],
) -> ProjectWorldModel {
    let mut nodes = Vec::new();
    let mut relationships = Vec::new();
    if repository.is_git {
        nodes.push(WorldNode {
            id: "repository".into(),
            kind: WorldNodeKind::Repository,
            label: "Git repository".into(),
            source: ".git".into(),
        });
    }
    nodes.extend(commands.iter().map(|command| WorldNode {
        id: format!("command:{}", command.name),
        kind: WorldNodeKind::Command,
        label: command.command.clone(),
        source: command.source.clone(),
    }));
    if repository.is_git {
        relationships.extend(commands.iter().map(|command| WorldRelationship {
            from: "repository".into(),
            relation: "provides".into(),
            to: format!("command:{}", command.name),
            source: command.source.clone(),
        }));
    }
    nodes.extend(
        services
            .iter()
            .enumerate()
            .map(|(index, service)| WorldNode {
                id: format!("service:{index}"),
                kind: WorldNodeKind::Service,
                label: service.clone(),
                source: "project manifest".into(),
            }),
    );
    nodes.extend(
        dependencies
            .iter()
            .enumerate()
            .map(|(index, (name, source))| WorldNode {
                id: format!("dependency:{index}"),
                kind: WorldNodeKind::Dependency,
                label: name.clone(),
                source: source.clone(),
            }),
    );
    if repository.is_git {
        relationships.extend(dependencies.iter().enumerate().map(|(index, (_, source))| {
            WorldRelationship {
                from: "repository".into(),
                relation: "depends_on".into(),
                to: format!("dependency:{index}"),
                source: source.clone(),
            }
        }));
    }
    ProjectWorldModel {
        nodes,
        relationships,
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fixture_repository_produces_stable_model_and_surfaces_instruction_conflict() {
        let root = tempdir().expect("root");
        fs::create_dir_all(root.path().join("src")).expect("src");
        fs::create_dir_all(root.path().join("packages/web")).expect("nested project");
        fs::create_dir_all(root.path().join(".cursor/rules")).expect("rules");
        fs::write(
            root.path().join("AGENTS.md"),
            "Use pnpm for package management.",
        )
        .expect("agents");
        fs::write(
            root.path().join(".cursor/rules/build.mdc"),
            "Use bun for package management.",
        )
        .expect("cursor");
        fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"build":"vite build","test":"vitest","test:e2e":"playwright test"},"devDependencies":{"vitest":"1","@playwright/test":"1"}}"#,
        )
        .expect("package");
        fs::write(root.path().join("pnpm-lock.yaml"), "lockfileVersion: 9").expect("lock");
        fs::write(root.path().join("src/main.ts"), "export {};").expect("source");
        fs::write(
            root.path().join("packages/web/AGENTS.md"),
            "Use bun for package management.",
        )
        .expect("nested agents");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.languages, vec!["TypeScript"]);
        assert!(first.test_systems.contains(&"Playwright".to_owned()));
        assert!(first.test_systems.contains(&"Vitest".to_owned()));
        assert!(first.package_managers.contains(&"pnpm".to_owned()));
        assert!(
            first
                .world
                .nodes
                .iter()
                .any(|node| node.kind == WorldNodeKind::Dependency && node.label == "vitest")
        );
        assert_eq!(first.instructions.conflicts.len(), 1);
        assert_eq!(first.instructions.effective[0].value, "pnpm");
        assert_eq!(
            first.instructions.conflicts[0].resolution,
            ConflictResolution::HigherPrecedence
        );
        assert!(first.commands.iter().any(|command| {
            command.name == "test:e2e" && command.command == "pnpm run test:e2e"
        }));
        assert_eq!(first.instructions.effective[0].value, "pnpm");
        let nested = InstructionResolver::for_target(
            &first.instructions.documents,
            "packages/web/src/main.ts",
        );
        assert_eq!(nested.effective[0].value, "bun");
        assert_eq!(
            nested.conflicts[0].resolution,
            ConflictResolution::MoreSpecific
        );
    }

    #[test]
    fn nested_repository_request_resolves_to_git_root_and_ignores_symlinks() {
        let root = tempdir().expect("root");
        fs::create_dir_all(root.path().join(".git")).expect("git");
        fs::create_dir_all(root.path().join("nested/project")).expect("nested");
        fs::write(root.path().join("Cargo.toml"), "[workspace]").expect("manifest");
        let model =
            ProjectDiscoverer::discover(&root.path().join("nested/project"), ProjectId::new())
                .expect("discover");
        assert_eq!(Path::new(&model.root), root.path().canonicalize().unwrap());
        assert!(model.build_systems.contains(&"Cargo".to_owned()));
    }

    #[test]
    fn polling_watch_invalidates_only_changed_manifest_model_areas() {
        let root = tempdir().expect("root");
        fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"build":"vite build"}}"#,
        )
        .expect("manifest");
        let mut watcher = ProjectWatcher::start(root.path(), ProjectId::new()).expect("watch");
        assert!(watcher.poll().expect("unchanged poll").is_none());
        fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"build":"vite build","test":"vitest"}}"#,
        )
        .expect("manifest update");
        let (model, invalidation) = watcher.poll().expect("changed poll").expect("change");
        assert_eq!(invalidation.changed_inputs, vec!["package.json"]);
        assert!(invalidation.invalidate_commands);
        assert!(!invalidation.invalidate_instructions);
        assert!(model.commands.iter().any(|command| command.name == "test"));
    }
}
