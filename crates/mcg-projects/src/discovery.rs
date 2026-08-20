use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use mcg_domain::{ProjectId, ProjectSnapshotId};
use quick_xml::{Reader, XmlVersion, events::Event};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;
use yaml_rust2::{Yaml, YamlLoader};

use crate::{
    ConflictResolution, EffectiveInstruction, InstructionConflict, InstructionDocument,
    InstructionKind, InstructionResolution, PROJECT_MODEL_SCHEMA_VERSION, ProjectCommand,
    ProjectModel, ProjectWorldModel, RecentCommit, RepositoryModel, SnapshotInvalidation,
    WorldNode, WorldNodeKind, WorldRelationship,
};

const MAX_FILES: usize = 20_000;
const MAX_DEPTH: usize = 12;
const MAX_INSTRUCTION_BYTES: u64 = 512 * 1024;
const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_WORLD_NODES: usize = 2_000;
const MAX_WORLD_RELATIONSHIPS: usize = 4_000;
const MAX_MANIFEST_ITEMS: usize = 500;

#[derive(Debug, Error)]
pub enum ProjectDiscoveryError {
    #[error("project path does not exist or is not a directory: {0}")]
    InvalidRoot(String),
    #[error("project discovery I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("project manifest is invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("project manifest is invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("project manifest is invalid YAML: {0}")]
    Yaml(String),
    #[error("project manifest is invalid XML: {0}")]
    Xml(String),
    #[error("project manifest exceeds the {MAX_MANIFEST_BYTES}-byte discovery limit: {0}")]
    ManifestTooLarge(String),
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
        let build_systems = discover_build_systems(&root, &files);
        let test_systems = discover_test_systems(&root, &files)?;
        let package_managers = discover_package_managers(&root, &files);
        let services = discover_services(&root, &files);
        let ci_systems = discover_ci(&files);
        let important_directories = discover_important_directories(&root);
        let existing_memory = files
            .iter()
            .find(|path| relative(&root, path) == "MEMORY.md")
            .map(|_| "MEMORY.md".to_owned());
        let dependencies = discover_dependencies(&root, &files)?;
        let world = world_model(
            &root,
            &files,
            &commands,
            &services,
            &repository,
            &dependencies,
        )?;

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
    candidates.sort_by_key(|right| std::cmp::Reverse(right.1.priority));
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
        let value: serde_json::Value = serde_json::from_str(&read_bounded_manifest(package_json)?)?;
        if let Some(scripts) = value.get("scripts").and_then(serde_json::Value::as_object) {
            let runner = preferred_js_runner(files);
            let preferred = ["build", "test", "check", "lint", "dev", "start", "test:e2e"];
            for name in preferred {
                if scripts.contains_key(name) {
                    commands.push(command(
                        name,
                        &format!("{runner} run {name}"),
                        "package.json",
                    ));
                }
            }
            for name in scripts
                .keys()
                .filter(|name| !preferred.contains(&name.as_str()))
                .filter(|name| safe_package_script_name(name))
            {
                commands.push(command(
                    name,
                    &format!("{runner} run {name}"),
                    "package.json",
                ));
            }
        }
    }
    if files.iter().any(|path| relative(root, path) == "Makefile") {
        commands.push(command("build", "make", "Makefile"));
        commands.push(command("test", "make test", "Makefile"));
    }
    if let Some(target) = dotnet_command_target(root, files) {
        commands.push(command(
            "build",
            &format!("dotnet build \"{target}\""),
            &target,
        ));
        commands.push(command(
            "test",
            &format!("dotnet test \"{target}\""),
            &target,
        ));
    }
    commands.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.command.cmp(&right.command))
    });
    commands.dedup_by(|left, right| left.name == right.name && left.command == right.command);
    Ok(commands)
}

fn safe_package_script_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'-'))
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

fn dotnet_command_target(root: &Path, files: &[PathBuf]) -> Option<String> {
    let mut candidates = files
        .iter()
        .map(|path| relative(root, path))
        .filter(|path| is_dotnet_solution(path) || is_dotnet_project(path))
        .filter(|path| safe_command_manifest_path(path))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| {
        (
            !is_dotnet_solution(path),
            path.matches('/').count(),
            path.clone(),
        )
    });
    candidates.into_iter().next()
}

fn safe_command_manifest_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'.' | b'_' | b'-' | b' ' | b'(' | b')')
        })
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
            | "vercel.json"
            | "netlify.toml"
            | "fly.toml"
            | "render.yaml"
            | "render.yml"
            | "wrangler.toml"
            | "railway.json"
    ) || path.starts_with(".github/workflows/")
        || is_dotnet_manifest(path)
        || is_openapi_manifest(path)
        || is_kubernetes_manifest(path)
        || is_helm_chart(path)
        || is_cloudformation_manifest(path)
        || is_terraform_json_manifest(path)
}

fn is_dotnet_manifest(path: &str) -> bool {
    is_dotnet_solution(path)
        || is_dotnet_project(path)
        || matches!(
            path,
            "global.json"
                | "Directory.Build.props"
                | "Directory.Build.targets"
                | "Directory.Packages.props"
        )
}

fn is_dotnet_solution(path: &str) -> bool {
    is_conventional_dotnet_path(path, &[".sln", ".slnx"])
}

fn is_dotnet_project(path: &str) -> bool {
    is_conventional_dotnet_path(path, &[".csproj", ".fsproj", ".vbproj"])
}

fn is_conventional_dotnet_path(path: &str, suffixes: &[&str]) -> bool {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    if !suffixes.iter().any(|suffix| lower.ends_with(suffix)) {
        return false;
    }
    let components = normalized.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| matches!(component.to_ascii_lowercase().as_str(), "bin" | "obj"))
    {
        return false;
    }
    components.len() == 1
        || components.first().is_some_and(|component| {
            matches!(
                component.to_ascii_lowercase().as_str(),
                "src" | "apps" | "packages" | "tests" | "services"
            )
        })
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

fn discover_build_systems(root: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut systems = named_presence(
        files,
        &[
            ("Cargo.toml", "Cargo"),
            ("package.json", "Node package scripts"),
            ("Makefile", "Make"),
            ("CMakeLists.txt", "CMake"),
            ("pyproject.toml", "Python pyproject"),
            ("go.mod", "Go modules"),
        ],
    );
    if files
        .iter()
        .any(|path| is_terraform_json_manifest(&relative(root, path)))
    {
        systems.push("Terraform JSON".into());
        systems.sort();
        systems.dedup();
    }
    if files
        .iter()
        .any(|path| is_dotnet_manifest(&relative(root, path)))
    {
        systems.push(".NET SDK".into());
        systems.sort();
        systems.dedup();
    }
    systems
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
        let value: serde_json::Value = serde_json::from_str(&read_bounded_manifest(package_json)?)?;
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
    if files
        .iter()
        .any(|path| is_dotnet_manifest(&relative(root, path)))
    {
        systems.insert("dotnet test".to_owned());
    }
    Ok(systems.into_iter().collect())
}

fn discover_package_managers(root: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut managers = named_presence(
        files,
        &[
            ("Cargo.lock", "Cargo"),
            ("package-lock.json", "npm"),
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "Yarn"),
            ("bun.lock", "Bun"),
            ("go.sum", "Go modules"),
        ],
    );
    if files
        .iter()
        .any(|path| is_dotnet_project(&relative(root, path)))
    {
        managers.push("NuGet".into());
        managers.sort();
        managers.dedup();
    }
    managers
}

fn discover_services(root: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut services = named_presence(
        files,
        &[
            ("Dockerfile", "Docker service"),
            ("docker-compose.yml", "Docker Compose"),
            ("docker-compose.yaml", "Docker Compose"),
            ("compose.yml", "Docker Compose"),
            ("compose.yaml", "Docker Compose"),
        ],
    );
    if files
        .iter()
        .any(|path| is_kubernetes_manifest(&relative(root, path)))
    {
        services.push("Kubernetes".into());
        services.sort();
        services.dedup();
    }
    if files
        .iter()
        .any(|path| is_helm_chart(&relative(root, path)))
    {
        services.push("Helm".into());
        services.sort();
        services.dedup();
    }
    services
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
    let value: serde_json::Value = serde_json::from_str(&read_bounded_manifest(package_json)?)?;
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
    root: &Path,
    files: &[PathBuf],
    commands: &[ProjectCommand],
    services: &[String],
    repository: &RepositoryModel,
    dependencies: &[(String, String)],
) -> Result<ProjectWorldModel, ProjectDiscoveryError> {
    let mut graph = WorldGraph::default();
    graph.add_fixed_node(
        "repository",
        WorldNodeKind::Repository,
        if repository.is_git {
            "Git repository"
        } else {
            "Project workspace"
        },
        if repository.is_git {
            ".git"
        } else {
            "project root"
        },
    );
    for command in commands {
        let id = graph.add_node(WorldNodeKind::Command, &command.command, &command.source);
        graph.add_relationship("repository", "provides", &id, &command.source);
    }
    for service in services {
        if matches!(service.as_str(), "Docker Compose" | "Kubernetes") {
            continue;
        }
        let id = graph.add_node(WorldNodeKind::Service, service, "project manifest");
        graph.add_relationship("repository", "contains", &id, "project manifest");
    }
    for (name, source) in dependencies {
        add_dependency(&mut graph, name, source);
    }
    add_runtime_and_package_facts(root, files, &mut graph)?;
    add_manifest_dependencies(root, files, &mut graph)?;
    add_dotnet_facts(root, files, &mut graph)?;
    add_compose_facts(root, files, &mut graph)?;
    add_kubernetes_facts(root, files, &mut graph)?;
    add_helm_facts(root, files, &mut graph)?;
    add_cloudformation_facts(root, files, &mut graph)?;
    add_terraform_json_facts(root, files, &mut graph)?;
    add_openapi_facts(root, files, &mut graph)?;
    add_deployment_facts(root, files, &mut graph);
    Ok(graph.finish())
}

#[derive(Default)]
struct WorldGraph {
    nodes: BTreeMap<String, WorldNode>,
    relationships: BTreeSet<(String, String, String, String)>,
}

impl WorldGraph {
    fn add_fixed_node(
        &mut self,
        id: &str,
        kind: WorldNodeKind,
        label: &str,
        source: &str,
    ) -> String {
        if self.nodes.len() < MAX_WORLD_NODES || self.nodes.contains_key(id) {
            self.nodes
                .entry(id.to_owned())
                .or_insert_with(|| WorldNode {
                    id: id.to_owned(),
                    kind,
                    label: label.to_owned(),
                    source: source.to_owned(),
                });
        }
        id.to_owned()
    }

    fn add_node(&mut self, kind: WorldNodeKind, label: &str, source: &str) -> String {
        let material = format!("{}\0{label}\0{source}", world_kind_name(kind));
        let digest = sha256(material.as_bytes());
        let id = format!("{}:{}", world_kind_name(kind), &digest[..16]);
        self.add_fixed_node(&id, kind, label, source)
    }

    fn add_relationship(&mut self, from: &str, relation: &str, to: &str, source: &str) {
        if self.relationships.len() >= MAX_WORLD_RELATIONSHIPS
            || !self.nodes.contains_key(from)
            || !self.nodes.contains_key(to)
        {
            return;
        }
        self.relationships.insert((
            from.to_owned(),
            relation.to_owned(),
            to.to_owned(),
            source.to_owned(),
        ));
    }

    fn finish(self) -> ProjectWorldModel {
        ProjectWorldModel {
            nodes: self.nodes.into_values().collect(),
            relationships: self
                .relationships
                .into_iter()
                .map(|(from, relation, to, source)| WorldRelationship {
                    from,
                    relation,
                    to,
                    source,
                })
                .collect(),
        }
    }
}

const fn world_kind_name(kind: WorldNodeKind) -> &'static str {
    match kind {
        WorldNodeKind::Service => "service",
        WorldNodeKind::Database => "database",
        WorldNodeKind::Domain => "domain",
        WorldNodeKind::Api => "api",
        WorldNodeKind::Deployment => "deployment",
        WorldNodeKind::Dependency => "dependency",
        WorldNodeKind::Repository => "repository",
        WorldNodeKind::Runtime => "runtime",
        WorldNodeKind::Command => "command",
    }
}

fn add_dependency(graph: &mut WorldGraph, name: &str, source: &str) {
    if name.trim().is_empty() || name.len() > 512 {
        return;
    }
    let id = graph.add_node(WorldNodeKind::Dependency, name, source);
    graph.add_relationship("repository", "depends_on", &id, source);
}

fn add_runtime_and_package_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    let runtime_manifests = [
        ("Cargo.toml", "Rust"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python"),
        ("go.mod", "Go"),
        ("pom.xml", "Java"),
        ("build.gradle", "Java/JVM"),
        ("build.gradle.kts", "Kotlin/JVM"),
    ];
    for (manifest, label) in runtime_manifests {
        if files.iter().any(|path| relative(root, path) == manifest) {
            let id = graph.add_node(WorldNodeKind::Runtime, label, manifest);
            graph.add_relationship("repository", "uses_runtime", &id, manifest);
        }
    }
    if let Some(source) = files
        .iter()
        .map(|path| relative(root, path))
        .filter(|path| is_dotnet_project(path))
        .min()
    {
        let id = graph.add_node(WorldNodeKind::Runtime, ".NET", &source);
        graph.add_relationship("repository", "uses_runtime", &id, &source);
    }
    let Some(package_json) = files
        .iter()
        .find(|path| relative(root, path) == "package.json")
    else {
        return Ok(());
    };
    let value: serde_json::Value = serde_json::from_str(&read_bounded_manifest(package_json)?)?;
    let node_label = value
        .pointer("/engines/node")
        .and_then(serde_json::Value::as_str)
        .map_or_else(
            || "Node.js".to_owned(),
            |version| format!("Node.js {version}"),
        );
    let runtime_id = graph.add_node(WorldNodeKind::Runtime, &node_label, "package.json#engines");
    graph.add_relationship("repository", "uses_runtime", &runtime_id, "package.json");
    let scripts = value.get("scripts").and_then(serde_json::Value::as_object);
    if scripts.is_some_and(|scripts| scripts.contains_key("start") || scripts.contains_key("dev")) {
        let label = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.trim().is_empty() && name.len() <= 256)
            .unwrap_or("Node application");
        let service_id = graph.add_node(WorldNodeKind::Service, label, "package.json#scripts");
        graph.add_relationship("repository", "provides", &service_id, "package.json");
        graph.add_relationship(&service_id, "runs_on", &runtime_id, "package.json#scripts");
        if let Some(homepage) = value
            .get("homepage")
            .and_then(serde_json::Value::as_str)
            .and_then(credential_free_origin)
        {
            let domain_id =
                graph.add_node(WorldNodeKind::Domain, &homepage, "package.json#homepage");
            graph.add_relationship(
                &service_id,
                "exposed_at",
                &domain_id,
                "package.json#homepage",
            );
        }
    }
    Ok(())
}

fn add_manifest_dependencies(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    if let Some(path) = files
        .iter()
        .find(|path| relative(root, path) == "Cargo.toml")
    {
        let value: toml::Value = toml::from_str(&read_bounded_manifest(path)?)?;
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(table) = value.get(section).and_then(toml::Value::as_table) {
                for name in table.keys().take(MAX_MANIFEST_ITEMS) {
                    add_dependency(graph, name, &format!("Cargo.toml#{section}"));
                }
            }
        }
        if let Some(table) = value
            .get("workspace")
            .and_then(|workspace| workspace.get("dependencies"))
            .and_then(toml::Value::as_table)
        {
            for name in table.keys().take(MAX_MANIFEST_ITEMS) {
                add_dependency(graph, name, "Cargo.toml#workspace.dependencies");
            }
        }
    }
    if let Some(path) = files
        .iter()
        .find(|path| relative(root, path) == "requirements.txt")
    {
        for line in read_bounded_manifest(path)?
            .lines()
            .take(MAX_MANIFEST_ITEMS)
        {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with('-')
                || line.contains('@')
            {
                continue;
            }
            let name = line
                .split(['=', '<', '>', '!', '~', '[', ';'])
                .next()
                .unwrap_or_default()
                .trim();
            add_dependency(graph, name, "requirements.txt");
        }
    }
    if let Some(path) = files.iter().find(|path| relative(root, path) == "go.mod") {
        let mut in_require = false;
        for line in read_bounded_manifest(path)?
            .lines()
            .take(MAX_MANIFEST_ITEMS * 2)
        {
            let line = line.trim();
            if line == "require (" {
                in_require = true;
                continue;
            }
            if in_require && line == ")" {
                in_require = false;
                continue;
            }
            let candidate = if in_require {
                line.split_whitespace().next()
            } else {
                line.strip_prefix("require ")
                    .and_then(|value| value.split_whitespace().next())
            };
            if let Some(name) = candidate.filter(|name| !name.is_empty() && !name.starts_with("//"))
            {
                add_dependency(graph, name, "go.mod#require");
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct DotnetProjectFacts {
    packages: BTreeSet<String>,
    frameworks: BTreeSet<String>,
    projects: BTreeSet<String>,
}

fn add_dotnet_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    let projects = files
        .iter()
        .filter_map(|path| {
            let source = relative(root, path);
            is_dotnet_project(&source).then_some((source, path))
        })
        .take(MAX_MANIFEST_ITEMS)
        .collect::<Vec<_>>();
    let mut nodes = BTreeMap::new();
    let mut facts = BTreeMap::new();
    for (source, path) in &projects {
        let label = Path::new(source)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| valid_dotnet_identifier(value))
            .unwrap_or(".NET project");
        let id = graph.add_node(WorldNodeKind::Service, label, source);
        graph.add_relationship("repository", "contains", &id, source);
        nodes.insert(source.clone(), id);
        facts.insert(source.clone(), parse_dotnet_project(path, source)?);
    }
    for (source, project_facts) in facts {
        let Some(project_id) = nodes.get(&source) else {
            continue;
        };
        for package in project_facts.packages {
            let dependency_id = graph.add_node(
                WorldNodeKind::Dependency,
                &package,
                &format!("{source}#PackageReference"),
            );
            graph.add_relationship(
                project_id,
                "depends_on",
                &dependency_id,
                &format!("{source}#PackageReference"),
            );
        }
        for framework in project_facts.frameworks {
            let dependency_id = graph.add_node(
                WorldNodeKind::Dependency,
                &framework,
                &format!("{source}#FrameworkReference"),
            );
            graph.add_relationship(
                project_id,
                "depends_on",
                &dependency_id,
                &format!("{source}#FrameworkReference"),
            );
        }
        for reference in project_facts.projects {
            if let Some(target_id) = resolve_dotnet_project_reference(&source, &reference)
                .and_then(|target| nodes.get(&target))
            {
                graph.add_relationship(
                    project_id,
                    "depends_on",
                    target_id,
                    &format!("{source}#ProjectReference"),
                );
            }
        }
    }
    Ok(())
}

fn parse_dotnet_project(
    path: &Path,
    source: &str,
) -> Result<DotnetProjectFacts, ProjectDiscoveryError> {
    let content = read_bounded_manifest(path)?;
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);
    let mut facts = DotnetProjectFacts::default();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element)) => {
                let local_name = element.local_name();
                let name = local_name.as_ref();
                let destination = if name.eq_ignore_ascii_case(b"PackageReference") {
                    Some(&mut facts.packages)
                } else if name.eq_ignore_ascii_case(b"FrameworkReference") {
                    Some(&mut facts.frameworks)
                } else if name.eq_ignore_ascii_case(b"ProjectReference") {
                    Some(&mut facts.projects)
                } else {
                    None
                };
                if let Some(destination) = destination {
                    for attribute in element.attributes().with_checks(true) {
                        let attribute = attribute.map_err(|error| {
                            ProjectDiscoveryError::Xml(format!("{source}: {error}"))
                        })?;
                        if !attribute
                            .key
                            .local_name()
                            .as_ref()
                            .eq_ignore_ascii_case(b"Include")
                        {
                            continue;
                        }
                        let value = attribute
                            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                            .map_err(|error| {
                                ProjectDiscoveryError::Xml(format!("{source}: {error}"))
                            })?;
                        let value = value.trim();
                        let valid = if name.eq_ignore_ascii_case(b"ProjectReference") {
                            valid_dotnet_reference(value)
                        } else {
                            valid_dotnet_identifier(value)
                        };
                        if valid {
                            destination.insert(value.to_owned());
                        }
                        break;
                    }
                }
            }
            Ok(Event::DocType(_)) => {
                return Err(ProjectDiscoveryError::Xml(format!(
                    "{source}: document types are not admitted"
                )));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(ProjectDiscoveryError::Xml(format!("{source}: {error}")));
            }
        }
    }
    Ok(facts)
}

fn valid_dotnet_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
}

fn valid_dotnet_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'\\' | b'.' | b'_' | b'-' | b' ' | b'(' | b')')
        })
}

fn resolve_dotnet_project_reference(source: &str, reference: &str) -> Option<String> {
    let mut components = source
        .rsplit_once('/')
        .map_or_else(Vec::new, |(parent, _)| {
            parent.split('/').map(str::to_owned).collect()
        });
    for component in reference.replace('\\', "/").split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop()?;
            }
            value => components.push(value.to_owned()),
        }
    }
    let normalized = components.join("/");
    is_dotnet_project(&normalized).then_some(normalized)
}

fn add_compose_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    for path in files.iter().filter(|path| {
        matches!(
            relative(root, path).as_str(),
            "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml"
        )
    }) {
        let source = relative(root, path);
        let documents = YamlLoader::load_from_str(&read_bounded_manifest(path)?)
            .map_err(|error| ProjectDiscoveryError::Yaml(error.to_string()))?;
        for document in documents.iter().take(4) {
            let Some(services) = yaml_field(document, "services").and_then(Yaml::as_hash) else {
                continue;
            };
            let container_runtime =
                graph.add_node(WorldNodeKind::Runtime, "OCI container", source.as_str());
            graph.add_relationship("repository", "uses_runtime", &container_runtime, &source);
            let mut service_ids = BTreeMap::new();
            let mut database_ids = BTreeMap::new();
            for (name, definition) in services.iter().take(256) {
                let Some(name) = name.as_str().filter(|name| valid_world_label(name)) else {
                    continue;
                };
                let fact_source = format!("{source}#services.{name}");
                let service_id = graph.add_node(WorldNodeKind::Service, name, &fact_source);
                graph.add_relationship("repository", "provides", &service_id, &fact_source);
                graph.add_relationship(&service_id, "runs_on", &container_runtime, &fact_source);
                service_ids.insert(name.to_owned(), service_id.clone());
                if let Some(image) = yaml_field(definition, "image")
                    .and_then(Yaml::as_str)
                    .filter(|image| valid_world_label(image))
                {
                    let image_id = graph.add_node(
                        WorldNodeKind::Dependency,
                        image,
                        &format!("{fact_source}.image"),
                    );
                    graph.add_relationship(&service_id, "uses_image", &image_id, &fact_source);
                    if let Some(database) = database_kind(image) {
                        let database_id = graph.add_node(
                            WorldNodeKind::Database,
                            &format!("{name} ({database})"),
                            &format!("{fact_source}.image"),
                        );
                        graph.add_relationship(&service_id, "provides", &database_id, &fact_source);
                        database_ids.insert(name.to_owned(), database_id);
                    }
                }
                if let Some(ports) = yaml_field(definition, "ports").and_then(Yaml::as_vec) {
                    for port in ports.iter().take(64).filter_map(compose_published_port) {
                        let endpoint = format!("localhost:{port}");
                        let domain_id = graph.add_node(
                            WorldNodeKind::Domain,
                            &endpoint,
                            &format!("{fact_source}.ports"),
                        );
                        graph.add_relationship(&service_id, "exposes", &domain_id, &fact_source);
                    }
                }
            }
            for (name, definition) in services.iter().take(256) {
                let Some(name) = name.as_str() else { continue };
                let Some(from) = service_ids.get(name) else {
                    continue;
                };
                for dependency in compose_dependencies(definition).into_iter().take(256) {
                    if let Some(to) = service_ids.get(&dependency) {
                        graph.add_relationship(
                            from,
                            "depends_on",
                            to,
                            &format!("{source}#services.{name}.depends_on"),
                        );
                    }
                    if let Some(database) = database_ids.get(&dependency) {
                        graph.add_relationship(
                            from,
                            "uses_database",
                            database,
                            &format!("{source}#services.{name}.depends_on"),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn yaml_field<'a>(value: &'a Yaml, field: &str) -> Option<&'a Yaml> {
    value
        .as_hash()?
        .iter()
        .find_map(|(key, value)| (key.as_str() == Some(field)).then_some(value))
}

fn compose_dependencies(definition: &Yaml) -> Vec<String> {
    let Some(value) = yaml_field(definition, "depends_on") else {
        return Vec::new();
    };
    let mut dependencies = if let Some(values) = value.as_vec() {
        values
            .iter()
            .filter_map(Yaml::as_str)
            .filter(|name| valid_world_label(name))
            .map(str::to_owned)
            .collect()
    } else if let Some(values) = value.as_hash() {
        values
            .keys()
            .filter_map(Yaml::as_str)
            .filter(|name| valid_world_label(name))
            .map(str::to_owned)
            .collect()
    } else {
        Vec::new()
    };
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn compose_published_port(value: &Yaml) -> Option<u16> {
    if let Some(published) = yaml_field(value, "published") {
        return published
            .as_i64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|port| *port > 0)
            .or_else(|| {
                published
                    .as_str()
                    .and_then(|value| value.parse::<u16>().ok())
                    .filter(|port| *port > 0)
            });
    }
    let rendered = value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_i64().map(|value| value.to_string()))?;
    let rendered = rendered.split('/').next()?;
    let parts = rendered.split(':').collect::<Vec<_>>();
    let published = if parts.len() >= 2 {
        parts[parts.len() - 2]
    } else {
        parts[0]
    };
    published.parse::<u16>().ok().filter(|port| *port > 0)
}

fn database_kind(image: &str) -> Option<&'static str> {
    let image = image.to_ascii_lowercase();
    [
        ("postgres", "PostgreSQL"),
        ("mariadb", "MariaDB"),
        ("mysql", "MySQL"),
        ("mongodb", "MongoDB"),
        ("mongo", "MongoDB"),
        ("redis", "Redis"),
        ("cockroach", "CockroachDB"),
        ("mssql", "SQL Server"),
        ("cassandra", "Cassandra"),
        ("neo4j", "Neo4j"),
    ]
    .into_iter()
    .find_map(|(needle, label)| image.contains(needle).then_some(label))
}

#[derive(Debug)]
struct KubernetesWorkload {
    namespace: String,
    labels: BTreeMap<String, String>,
    service_id: String,
}

#[derive(Debug)]
struct KubernetesEndpoint {
    namespace: String,
    name: String,
    selector: BTreeMap<String, String>,
    domain_ids: Vec<String>,
}

type KubernetesIngressBackend = (String, String, String, String);

struct KubernetesFacts<'a> {
    graph: &'a mut WorldGraph,
    workloads: Vec<KubernetesWorkload>,
    endpoints: Vec<KubernetesEndpoint>,
    ingress_backends: Vec<KubernetesIngressBackend>,
}

impl<'a> KubernetesFacts<'a> {
    fn new(graph: &'a mut WorldGraph) -> Self {
        Self {
            graph,
            workloads: Vec::new(),
            endpoints: Vec::new(),
            ingress_backends: Vec::new(),
        }
    }

    fn add_document(&mut self, document: &Yaml, fact_source: &str) {
        let Some(kind) = yaml_field(document, "kind")
            .and_then(Yaml::as_str)
            .filter(|kind| valid_world_label(kind))
        else {
            return;
        };
        let Some(metadata) = yaml_field(document, "metadata") else {
            return;
        };
        let Some(name) = yaml_field(metadata, "name")
            .and_then(Yaml::as_str)
            .filter(|name| valid_kubernetes_name(name))
        else {
            return;
        };
        let namespace = yaml_field(metadata, "namespace")
            .and_then(Yaml::as_str)
            .filter(|namespace| valid_kubernetes_name(namespace))
            .unwrap_or("default");
        match kind {
            "Deployment" | "StatefulSet" | "DaemonSet" | "Job" | "CronJob" => {
                self.add_workload(document, kind, namespace, name, fact_source);
            }
            "Service" => self.add_service(document, namespace, name, fact_source),
            "Ingress" => self.ingress_backends.extend(kubernetes_ingress_backends(
                document,
                namespace,
                fact_source,
                self.graph,
            )),
            _ => {}
        }
    }

    fn add_workload(
        &mut self,
        document: &Yaml,
        kind: &str,
        namespace: &str,
        name: &str,
        fact_source: &str,
    ) {
        let Some(pod_template) = kubernetes_pod_template(document, kind) else {
            return;
        };
        let label = if namespace == "default" {
            name.to_owned()
        } else {
            format!("{namespace}/{name}")
        };
        let service_id = self
            .graph
            .add_node(WorldNodeKind::Service, &label, fact_source);
        self.graph
            .add_relationship("repository", "provides", &service_id, fact_source);
        let deployment_id = self.graph.add_node(
            WorldNodeKind::Deployment,
            &format!("Kubernetes {kind} {namespace}/{name}"),
            fact_source,
        );
        self.graph
            .add_relationship("repository", "deploys_to", &deployment_id, fact_source);
        self.graph
            .add_relationship(&service_id, "deployed_as", &deployment_id, fact_source);
        let runtime_id = self
            .graph
            .add_node(WorldNodeKind::Runtime, "OCI container", fact_source);
        self.graph
            .add_relationship(&service_id, "runs_on", &runtime_id, fact_source);
        if let Some(spec) = yaml_field(pod_template, "spec") {
            self.add_container_images(&service_id, &label, spec, fact_source);
        }
        self.workloads.push(KubernetesWorkload {
            namespace: namespace.to_owned(),
            labels: yaml_field(pod_template, "metadata")
                .and_then(|metadata| yaml_field(metadata, "labels"))
                .map(kubernetes_string_map)
                .unwrap_or_default(),
            service_id,
        });
    }

    fn add_container_images(
        &mut self,
        service_id: &str,
        label: &str,
        spec: &Yaml,
        fact_source: &str,
    ) {
        for image in kubernetes_container_images(spec) {
            let image_source = format!("{fact_source}.containers");
            let image_id = self
                .graph
                .add_node(WorldNodeKind::Dependency, &image, &image_source);
            self.graph
                .add_relationship(service_id, "uses_image", &image_id, fact_source);
            if let Some(database) = database_kind(&image) {
                let database_id = self.graph.add_node(
                    WorldNodeKind::Database,
                    &format!("{label} ({database})"),
                    &image_source,
                );
                self.graph
                    .add_relationship(service_id, "provides", &database_id, fact_source);
            }
        }
    }

    fn add_service(&mut self, document: &Yaml, namespace: &str, name: &str, fact_source: &str) {
        let Some(spec) = yaml_field(document, "spec") else {
            return;
        };
        let selector = yaml_field(spec, "selector")
            .map(kubernetes_string_map)
            .unwrap_or_default();
        let mut domain_ids = Vec::new();
        if let Some(ports) = yaml_field(spec, "ports").and_then(Yaml::as_vec) {
            for port in ports.iter().take(64).filter_map(kubernetes_service_port) {
                let domain = format!("k8s://{namespace}/{name}:{port}");
                let domain_id = self.graph.add_node(
                    WorldNodeKind::Domain,
                    &domain,
                    &format!("{fact_source}.spec.ports"),
                );
                self.graph
                    .add_relationship("repository", "exposes", &domain_id, fact_source);
                domain_ids.push(domain_id);
            }
        }
        self.endpoints.push(KubernetesEndpoint {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            selector,
            domain_ids,
        });
    }

    fn connect_relationships(&mut self) {
        for endpoint in &self.endpoints {
            for workload in self.workloads.iter().filter(|workload| {
                workload.namespace == endpoint.namespace
                    && !endpoint.selector.is_empty()
                    && endpoint
                        .selector
                        .iter()
                        .all(|(key, value)| workload.labels.get(key) == Some(value))
            }) {
                for domain_id in &endpoint.domain_ids {
                    self.graph.add_relationship(
                        &workload.service_id,
                        "exposed_at",
                        domain_id,
                        &format!(
                            "Kubernetes Service {}/{}",
                            endpoint.namespace, endpoint.name
                        ),
                    );
                }
            }
        }
        for (namespace, service_name, ingress_domain_id, source) in &self.ingress_backends {
            let matched = self
                .endpoints
                .iter()
                .filter(|endpoint| {
                    endpoint.namespace == *namespace && endpoint.name == *service_name
                })
                .flat_map(|endpoint| endpoint.domain_ids.iter());
            for endpoint_domain_id in matched {
                self.graph.add_relationship(
                    ingress_domain_id,
                    "routes_to",
                    endpoint_domain_id,
                    source,
                );
            }
        }
    }
}

fn add_kubernetes_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    let mut facts = KubernetesFacts::new(graph);
    for path in files
        .iter()
        .filter(|path| is_kubernetes_manifest(&relative(root, path)))
    {
        let source = relative(root, path);
        let documents = YamlLoader::load_from_str(&read_bounded_manifest(path)?)
            .map_err(|error| ProjectDiscoveryError::Yaml(error.to_string()))?;
        for (document_index, document) in documents.iter().take(64).enumerate() {
            let fact_source = format!("{source}#document-{document_index}");
            facts.add_document(document, &fact_source);
        }
    }
    facts.connect_relationships();
    Ok(())
}

fn is_kubernetes_manifest(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    let yaml = Path::new(file_name)
        .extension()
        .is_some_and(|extension| matches!(extension.to_str(), Some("yaml" | "yml")));
    yaml && (matches!(
        file_name,
        "k8s.yaml" | "k8s.yml" | "kubernetes.yaml" | "kubernetes.yml"
    ) || normalized.starts_with("k8s/")
        || normalized.starts_with("kubernetes/")
        || normalized.starts_with("deploy/kubernetes/")
        || normalized.starts_with("manifests/kubernetes/"))
}

fn kubernetes_pod_template<'a>(document: &'a Yaml, kind: &str) -> Option<&'a Yaml> {
    let spec = yaml_field(document, "spec")?;
    if kind == "CronJob" {
        return yaml_field(spec, "jobTemplate")
            .and_then(|job| yaml_field(job, "spec"))
            .and_then(|job_spec| yaml_field(job_spec, "template"));
    }
    yaml_field(spec, "template")
}

fn kubernetes_container_images(spec: &Yaml) -> Vec<String> {
    ["initContainers", "containers"]
        .into_iter()
        .flat_map(|field| {
            yaml_field(spec, field)
                .and_then(Yaml::as_vec)
                .into_iter()
                .flatten()
                .take(128)
        })
        .filter_map(|container| yaml_field(container, "image").and_then(Yaml::as_str))
        .filter(|image| valid_world_label(image))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn kubernetes_string_map(value: &Yaml) -> BTreeMap<String, String> {
    value
        .as_hash()
        .into_iter()
        .flatten()
        .take(128)
        .filter_map(|(key, value)| {
            let key = key.as_str().filter(|key| valid_kubernetes_label(key))?;
            let value = value
                .as_str()
                .filter(|value| valid_kubernetes_label(value))?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn kubernetes_service_port(value: &Yaml) -> Option<u16> {
    let port = yaml_field(value, "port")?;
    port.as_i64()
        .and_then(|port| u16::try_from(port).ok())
        .or_else(|| port.as_str().and_then(|port| port.parse::<u16>().ok()))
        .filter(|port| *port > 0)
}

fn kubernetes_ingress_backends(
    document: &Yaml,
    namespace: &str,
    fact_source: &str,
    graph: &mut WorldGraph,
) -> Vec<(String, String, String, String)> {
    let Some(spec) = yaml_field(document, "spec") else {
        return Vec::new();
    };
    let tls_hosts = yaml_field(spec, "tls")
        .and_then(Yaml::as_vec)
        .into_iter()
        .flatten()
        .take(64)
        .flat_map(|entry| {
            yaml_field(entry, "hosts")
                .and_then(Yaml::as_vec)
                .into_iter()
                .flatten()
                .take(64)
        })
        .filter_map(Yaml::as_str)
        .filter(|host| valid_kubernetes_host(host))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    let Some(rules) = yaml_field(spec, "rules").and_then(Yaml::as_vec) else {
        return result;
    };
    for rule in rules.iter().take(128) {
        let Some(host) = yaml_field(rule, "host")
            .and_then(Yaml::as_str)
            .filter(|host| valid_kubernetes_host(host))
        else {
            continue;
        };
        let scheme = if tls_hosts.contains(host) {
            "https"
        } else {
            "http"
        };
        let origin = format!("{scheme}://{host}");
        let domain_id = graph.add_node(WorldNodeKind::Domain, &origin, fact_source);
        graph.add_relationship("repository", "exposes", &domain_id, fact_source);
        let paths = yaml_field(rule, "http")
            .and_then(|http| yaml_field(http, "paths"))
            .and_then(Yaml::as_vec);
        for path in paths.into_iter().flatten().take(128) {
            let Some(backend) = yaml_field(path, "backend") else {
                continue;
            };
            let service_name = yaml_field(backend, "service")
                .and_then(|service| yaml_field(service, "name"))
                .and_then(Yaml::as_str)
                .or_else(|| yaml_field(backend, "serviceName").and_then(Yaml::as_str));
            if let Some(service_name) = service_name.filter(|name| valid_kubernetes_name(name)) {
                result.push((
                    namespace.to_owned(),
                    service_name.to_owned(),
                    domain_id.clone(),
                    fact_source.to_owned(),
                ));
            }
        }
    }
    result
}

fn valid_kubernetes_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn valid_kubernetes_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && !value.chars().any(char::is_control)
        && !value.to_ascii_lowercase().contains("password")
        && !value.to_ascii_lowercase().contains("token")
}

fn valid_kubernetes_host(value: &str) -> bool {
    valid_kubernetes_name(value) && value.contains('.')
}

fn add_helm_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    for path in files
        .iter()
        .filter(|path| is_helm_chart(&relative(root, path)))
    {
        let source = relative(root, path);
        let documents = YamlLoader::load_from_str(&read_bounded_manifest(path)?)
            .map_err(|error| ProjectDiscoveryError::Yaml(error.to_string()))?;
        let Some(document) = documents.first() else {
            continue;
        };
        let api_version = yaml_field(document, "apiVersion").and_then(Yaml::as_str);
        if !matches!(api_version, Some("v1" | "v2")) {
            continue;
        }
        let Some(name) = yaml_field(document, "name")
            .and_then(Yaml::as_str)
            .filter(|name| valid_helm_name(name))
        else {
            continue;
        };
        let chart_source = format!("{source}#name");
        let chart_id = graph.add_node(
            WorldNodeKind::Deployment,
            &format!("Helm chart {name}"),
            &chart_source,
        );
        graph.add_relationship("repository", "deploys_to", &chart_id, &chart_source);

        let Some(dependencies) = yaml_field(document, "dependencies").and_then(Yaml::as_vec) else {
            continue;
        };
        for dependency in dependencies.iter().take(MAX_MANIFEST_ITEMS) {
            let Some(dependency_name) = yaml_field(dependency, "name")
                .and_then(Yaml::as_str)
                .filter(|name| valid_helm_name(name))
            else {
                continue;
            };
            let dependency_source = format!("{source}#dependencies.{dependency_name}");
            let dependency_id = graph.add_node(
                WorldNodeKind::Dependency,
                dependency_name,
                &dependency_source,
            );
            graph.add_relationship(&chart_id, "depends_on", &dependency_id, &dependency_source);
            if let Some(database) = database_kind(dependency_name) {
                let database_id = graph.add_node(
                    WorldNodeKind::Database,
                    &format!("{dependency_name} ({database})"),
                    &dependency_source,
                );
                graph.add_relationship(
                    &chart_id,
                    "uses_database",
                    &database_id,
                    &dependency_source,
                );
            }
        }
    }
    Ok(())
}

fn is_helm_chart(path: &str) -> bool {
    let segments = path.split('/').collect::<Vec<_>>();
    matches!(
        segments.as_slice(),
        ["Chart.yaml"]
            | ["helm" | "charts", _, "Chart.yaml"]
            | ["deploy" | "manifests", "helm", _, "Chart.yaml"]
    )
}

fn valid_helm_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 53
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

#[derive(Debug)]
struct CloudFormationResource {
    logical_id: String,
    type_name: String,
    dependencies: Vec<String>,
}

fn add_cloudformation_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    for path in files
        .iter()
        .filter(|path| is_cloudformation_manifest(&relative(root, path)))
    {
        let source = relative(root, path);
        let text = read_bounded_manifest(path)?;
        if ambiguous_root_cloudformation_template(&source)
            && !(text.contains("Resources") && text.contains("AWS::"))
        {
            continue;
        }
        let resources = cloudformation_resources(path, &text)?;
        if resources.is_empty() {
            continue;
        }
        let stack_id = graph.add_node(
            WorldNodeKind::Deployment,
            &format!("CloudFormation stack {source}"),
            &source,
        );
        graph.add_relationship("repository", "deploys_to", &stack_id, &source);
        let mut resource_ids = BTreeMap::new();
        for resource in &resources {
            let fact_source = format!("{source}#Resources.{}", resource.logical_id);
            let resource_id = graph.add_node(
                cloudformation_world_kind(&resource.type_name),
                &format!("{} ({})", resource.logical_id, resource.type_name),
                &fact_source,
            );
            graph.add_relationship(&stack_id, "provisions", &resource_id, &fact_source);
            resource_ids.insert(resource.logical_id.clone(), resource_id);
        }
        for resource in &resources {
            let Some(from) = resource_ids.get(&resource.logical_id) else {
                continue;
            };
            for dependency in &resource.dependencies {
                if let Some(to) = resource_ids.get(dependency) {
                    graph.add_relationship(
                        from,
                        "depends_on",
                        to,
                        &format!("{source}#Resources.{}.DependsOn", resource.logical_id),
                    );
                }
            }
        }
    }
    Ok(())
}

fn cloudformation_resources(
    path: &Path,
    text: &str,
) -> Result<Vec<CloudFormationResource>, ProjectDiscoveryError> {
    if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        let document: serde_json::Value = serde_json::from_str(text)?;
        let Some(resources) = document
            .get("Resources")
            .and_then(serde_json::Value::as_object)
        else {
            return Ok(Vec::new());
        };
        return Ok(resources
            .iter()
            .take(MAX_MANIFEST_ITEMS)
            .filter_map(|(logical_id, definition)| {
                let logical_id =
                    valid_cloudformation_logical_id(logical_id).then(|| logical_id.clone())?;
                let type_name = definition
                    .get("Type")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| valid_cloudformation_type(value))?
                    .to_owned();
                let dependencies = definition
                    .get("DependsOn")
                    .map(json_cloudformation_dependencies)
                    .unwrap_or_default();
                Some(CloudFormationResource {
                    logical_id,
                    type_name,
                    dependencies,
                })
            })
            .collect());
    }
    let documents = YamlLoader::load_from_str(text)
        .map_err(|error| ProjectDiscoveryError::Yaml(error.to_string()))?;
    let Some(document) = documents.first() else {
        return Ok(Vec::new());
    };
    let Some(resources) = yaml_field(document, "Resources").and_then(Yaml::as_hash) else {
        return Ok(Vec::new());
    };
    Ok(resources
        .iter()
        .take(MAX_MANIFEST_ITEMS)
        .filter_map(|(logical_id, definition)| {
            let logical_id = logical_id
                .as_str()
                .filter(|value| valid_cloudformation_logical_id(value))?
                .to_owned();
            let type_name = yaml_field(definition, "Type")
                .and_then(Yaml::as_str)
                .filter(|value| valid_cloudformation_type(value))?
                .to_owned();
            let dependencies = yaml_field(definition, "DependsOn")
                .map(yaml_cloudformation_dependencies)
                .unwrap_or_default();
            Some(CloudFormationResource {
                logical_id,
                type_name,
                dependencies,
            })
        })
        .collect())
}

fn json_cloudformation_dependencies(value: &serde_json::Value) -> Vec<String> {
    let values = value
        .as_array()
        .map_or_else(|| std::slice::from_ref(value), Vec::as_slice);
    bounded_cloudformation_dependencies(values.iter().filter_map(serde_json::Value::as_str))
}

fn yaml_cloudformation_dependencies(value: &Yaml) -> Vec<String> {
    let values = value
        .as_vec()
        .map_or_else(|| std::slice::from_ref(value), Vec::as_slice);
    bounded_cloudformation_dependencies(values.iter().filter_map(Yaml::as_str))
}

fn bounded_cloudformation_dependencies<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut dependencies = values
        .take(MAX_MANIFEST_ITEMS)
        .filter(|value| valid_cloudformation_logical_id(value))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn is_cloudformation_manifest(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    let name = normalized.rsplit('/').next().unwrap_or(&normalized);
    let conventional_name = matches!(
        name,
        "template.yaml"
            | "template.yml"
            | "template.json"
            | "sam-template.yaml"
            | "sam-template.yml"
            | "cloudformation.yaml"
            | "cloudformation.yml"
            | "cloudformation.json"
    );
    conventional_name
        && (!normalized.contains('/')
            || normalized.starts_with("cloudformation/")
            || normalized.starts_with("infra/cloudformation/")
            || normalized.starts_with("deploy/cloudformation/"))
}

fn valid_cloudformation_logical_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_cloudformation_type(value: &str) -> bool {
    if value.len() > 256 {
        return false;
    }
    let parts = value.split("::").collect::<Vec<_>>();
    parts.len() == 3
        && parts[0] == "AWS"
        && parts[1..]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

fn ambiguous_root_cloudformation_template(path: &str) -> bool {
    !path.contains('/')
        && matches!(
            path.to_ascii_lowercase().as_str(),
            "template.yaml" | "template.yml" | "template.json"
        )
}

fn cloudformation_world_kind(type_name: &str) -> WorldNodeKind {
    if [
        "AWS::RDS::",
        "AWS::DynamoDB::",
        "AWS::ElastiCache::",
        "AWS::Neptune::",
        "AWS::DocumentDB::",
        "AWS::Timestream::",
    ]
    .iter()
    .any(|prefix| type_name.starts_with(prefix))
    {
        WorldNodeKind::Database
    } else if type_name.starts_with("AWS::ApiGateway::")
        || type_name.starts_with("AWS::ApiGatewayV2::")
        || type_name == "AWS::AppSync::GraphQLApi"
    {
        WorldNodeKind::Api
    } else if matches!(
        type_name,
        "AWS::Lambda::Function"
            | "AWS::Serverless::Function"
            | "AWS::ECS::Service"
            | "AWS::ElasticBeanstalk::Application"
            | "AWS::AppRunner::Service"
    ) {
        WorldNodeKind::Service
    } else {
        WorldNodeKind::Deployment
    }
}

#[derive(Debug)]
struct TerraformResource {
    address: String,
    type_name: String,
    name: String,
    dependencies: Vec<String>,
}

fn add_terraform_json_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    for path in files
        .iter()
        .filter(|path| is_terraform_json_manifest(&relative(root, path)))
    {
        let source = relative(root, path);
        let document: serde_json::Value = serde_json::from_str(&read_bounded_manifest(path)?)?;
        let providers = terraform_required_providers(&document);
        let resources = terraform_resources(&document);
        if providers.is_empty() && resources.is_empty() {
            continue;
        }
        let configuration_id = graph.add_node(
            WorldNodeKind::Deployment,
            &format!("Terraform configuration {source}"),
            &source,
        );
        graph.add_relationship("repository", "deploys_to", &configuration_id, &source);

        let mut provider_ids = BTreeMap::new();
        for (local_name, provider_source) in providers {
            let fact_source = format!("{source}#terraform.required_providers.{local_name}");
            let provider_id = graph.add_node(
                WorldNodeKind::Dependency,
                &format!("Terraform provider {provider_source}"),
                &fact_source,
            );
            graph.add_relationship(&configuration_id, "depends_on", &provider_id, &fact_source);
            provider_ids.insert(local_name, provider_id);
        }

        let mut resource_ids = BTreeMap::new();
        for resource in &resources {
            let fact_source = format!("{source}#{}", resource.address);
            let label = if resource.type_name == "module" {
                format!("Terraform module {}", resource.name)
            } else {
                format!("{} ({})", resource.name, resource.type_name)
            };
            let resource_id = graph.add_node(
                terraform_world_kind(&resource.type_name),
                &label,
                &fact_source,
            );
            graph.add_relationship(&configuration_id, "provisions", &resource_id, &fact_source);
            if let Some(local_provider) = resource.type_name.split('_').next()
                && let Some(provider_id) = provider_ids.get(local_provider)
            {
                graph.add_relationship(&resource_id, "uses_provider", provider_id, &fact_source);
            }
            resource_ids.insert(resource.address.clone(), resource_id);
        }
        for resource in &resources {
            let Some(from) = resource_ids.get(&resource.address) else {
                continue;
            };
            for dependency in &resource.dependencies {
                if let Some(to) = resource_ids.get(dependency) {
                    graph.add_relationship(
                        from,
                        "depends_on",
                        to,
                        &format!("{source}#{}.depends_on", resource.address),
                    );
                }
            }
        }
    }
    Ok(())
}

fn terraform_required_providers(document: &serde_json::Value) -> BTreeMap<String, String> {
    document
        .pointer("/terraform/required_providers")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|providers| providers.iter())
        .take(MAX_MANIFEST_ITEMS)
        .filter_map(|(local_name, definition)| {
            if !valid_terraform_name(local_name) {
                return None;
            }
            let source = definition
                .get("source")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(local_name);
            valid_terraform_provider_source(source).then(|| (local_name.clone(), source.to_owned()))
        })
        .collect()
}

fn terraform_resources(document: &serde_json::Value) -> Vec<TerraformResource> {
    let mut resources = Vec::new();
    if let Some(types) = document
        .get("resource")
        .and_then(serde_json::Value::as_object)
    {
        for (type_name, instances) in types.iter().take(MAX_MANIFEST_ITEMS) {
            if resources.len() >= MAX_MANIFEST_ITEMS {
                break;
            }
            if !valid_terraform_name(type_name) {
                continue;
            }
            let Some(instances) = instances.as_object() else {
                continue;
            };
            for (name, definition) in instances {
                if resources.len() >= MAX_MANIFEST_ITEMS {
                    break;
                }
                if !valid_terraform_name(name) || !definition.is_object() {
                    continue;
                }
                resources.push(TerraformResource {
                    address: format!("{type_name}.{name}"),
                    type_name: type_name.clone(),
                    name: name.clone(),
                    dependencies: terraform_dependencies(definition),
                });
            }
        }
    }
    if let Some(modules) = document
        .get("module")
        .and_then(serde_json::Value::as_object)
    {
        for (name, definition) in modules.iter().take(MAX_MANIFEST_ITEMS) {
            if resources.len() >= MAX_MANIFEST_ITEMS {
                break;
            }
            if !valid_terraform_name(name) || !definition.is_object() {
                continue;
            }
            resources.push(TerraformResource {
                address: format!("module.{name}"),
                type_name: "module".into(),
                name: name.clone(),
                dependencies: terraform_dependencies(definition),
            });
        }
    }
    resources.sort_by(|left, right| left.address.cmp(&right.address));
    resources
}

fn terraform_dependencies(definition: &serde_json::Value) -> Vec<String> {
    let mut dependencies = definition
        .get("depends_on")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .take(MAX_MANIFEST_ITEMS)
        .filter_map(serde_json::Value::as_str)
        .filter(|value| valid_terraform_address(value))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn is_terraform_json_manifest(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    if !normalized.ends_with(".tf.json") {
        return false;
    }
    let segments = normalized.split('/').collect::<Vec<_>>();
    !segments.contains(&".terraform")
        && (segments.len() == 1
            || matches!(segments.first(), Some(&"infra" | &"terraform" | &"iac")))
}

fn valid_terraform_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_terraform_provider_source(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/'))
}

fn valid_terraform_address(value: &str) -> bool {
    if value.len() > 260 || value.chars().any(char::is_control) {
        return false;
    }
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 2 && parts.iter().all(|part| valid_terraform_name(part))
}

fn terraform_world_kind(type_name: &str) -> WorldNodeKind {
    if type_name == "module" {
        return WorldNodeKind::Deployment;
    }
    if type_name.starts_with("aws_db_")
        || type_name.starts_with("aws_rds_")
        || type_name.starts_with("aws_dynamodb_")
        || type_name.starts_with("aws_elasticache_")
        || type_name.starts_with("google_sql_")
        || type_name.starts_with("azurerm_postgresql_")
        || type_name.starts_with("azurerm_mysql_")
        || type_name.starts_with("azurerm_cosmosdb_")
    {
        WorldNodeKind::Database
    } else if type_name.starts_with("aws_api_gateway_")
        || type_name.starts_with("aws_apigatewayv2_")
        || type_name.starts_with("google_api_gateway_")
        || type_name.starts_with("azurerm_api_management_")
    {
        WorldNodeKind::Api
    } else if matches!(
        type_name,
        "aws_lambda_function"
            | "aws_ecs_service"
            | "google_cloud_run_service"
            | "google_cloudfunctions_function"
            | "azurerm_linux_web_app"
            | "azurerm_windows_web_app"
            | "kubernetes_deployment"
    ) {
        WorldNodeKind::Service
    } else {
        WorldNodeKind::Deployment
    }
}

fn add_openapi_facts(
    root: &Path,
    files: &[PathBuf],
    graph: &mut WorldGraph,
) -> Result<(), ProjectDiscoveryError> {
    for path in files
        .iter()
        .filter(|path| is_openapi_manifest(&relative(root, path)))
    {
        let source = relative(root, path);
        let text = read_bounded_manifest(path)?;
        let (title, servers) = if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let value: serde_json::Value = serde_json::from_str(&text)?;
            let title = value
                .pointer("/info/title")
                .and_then(serde_json::Value::as_str)
                .filter(|title| valid_world_label(title))
                .unwrap_or(source.as_str())
                .to_owned();
            let servers: Vec<String> = value
                .get("servers")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .take(32)
                .filter_map(|server| server.get("url").and_then(serde_json::Value::as_str))
                .filter_map(credential_free_origin)
                .collect();
            (title, servers)
        } else {
            let documents = YamlLoader::load_from_str(&text)
                .map_err(|error| ProjectDiscoveryError::Yaml(error.to_string()))?;
            let Some(document) = documents.first() else {
                continue;
            };
            let title = yaml_field(document, "info")
                .and_then(|info| yaml_field(info, "title"))
                .and_then(Yaml::as_str)
                .filter(|title| valid_world_label(title))
                .unwrap_or(source.as_str())
                .to_owned();
            let servers: Vec<String> = yaml_field(document, "servers")
                .and_then(Yaml::as_vec)
                .into_iter()
                .flatten()
                .take(32)
                .filter_map(|server| yaml_field(server, "url").and_then(Yaml::as_str))
                .filter_map(credential_free_origin)
                .collect();
            (title, servers)
        };
        let api_id = graph.add_node(WorldNodeKind::Api, &title, &source);
        graph.add_relationship("repository", "describes", &api_id, &source);
        for origin in servers {
            let domain_id = graph.add_node(WorldNodeKind::Domain, &origin, &source);
            graph.add_relationship(&api_id, "served_at", &domain_id, &source);
        }
    }
    Ok(())
}

fn is_openapi_manifest(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    matches!(
        name.as_str(),
        "openapi.json"
            | "openapi.yaml"
            | "openapi.yml"
            | "swagger.json"
            | "swagger.yaml"
            | "swagger.yml"
    )
}

fn credential_free_origin(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn add_deployment_facts(root: &Path, files: &[PathBuf], graph: &mut WorldGraph) {
    let manifests = [
        ("vercel.json", "Vercel"),
        ("netlify.toml", "Netlify"),
        ("fly.toml", "Fly.io"),
        ("render.yaml", "Render"),
        ("render.yml", "Render"),
        ("wrangler.toml", "Cloudflare Workers"),
        ("railway.json", "Railway"),
        ("Dockerfile", "Docker image"),
    ];
    for (manifest, label) in manifests {
        if files.iter().any(|path| relative(root, path) == manifest) {
            let id = graph.add_node(WorldNodeKind::Deployment, label, manifest);
            graph.add_relationship("repository", "deploys_to", &id, manifest);
        }
    }
}

fn valid_world_label(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 512
        && !value.chars().any(char::is_control)
        && !value.to_ascii_lowercase().contains("password")
        && !value.to_ascii_lowercase().contains("token=")
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_bounded_manifest(path: &Path) -> Result<String, ProjectDiscoveryError> {
    if path.metadata()?.len() > MAX_MANIFEST_BYTES {
        return Err(ProjectDiscoveryError::ManifestTooLarge(
            path.display().to_string(),
        ));
    }
    Ok(fs::read_to_string(path)?)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TopologyPlanStatus;
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
            r#"{"scripts":{"build":"vite build","test":"vitest","test:e2e":"playwright test","verify-artifact":"node verify.mjs","unsafe script":"node unsafe.mjs"},"devDependencies":{"vitest":"1","@playwright/test":"1"}}"#,
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
        assert!(first.commands.iter().any(|command| {
            command.name == "verify-artifact" && command.command == "pnpm run verify-artifact"
        }));
        assert!(
            !first
                .commands
                .iter()
                .any(|command| command.name == "unsafe script")
        );
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
    #[allow(clippy::too_many_lines)] // One topology fixture keeps node and edge provenance assertions together.
    fn manifest_world_model_is_typed_stable_provenance_bearing_and_secret_safe() {
        let root = tempdir().expect("root");
        fs::write(
            root.path().join("package.json"),
            r#"{"name":"orders-web","homepage":"https://orders.example.test/app","scripts":{"start":"node server.js"},"dependencies":{"fastify":"5"}}"#,
        )
        .expect("package manifest");
        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"orders-worker\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .expect("cargo manifest");
        fs::write(
            root.path().join("requirements.txt"),
            "requests==2.32\nprivate @ https://owner:secret@example.test/private.whl\n",
        )
        .expect("Python dependencies");
        fs::write(
            root.path().join("go.mod"),
            "module example.test/orders\n\nrequire example.test/observability v1.2.3\n",
        )
        .expect("Go dependencies");
        fs::write(
            root.path().join("compose.yaml"),
            r#"services:
  web:
    image: ghcr.io/example/orders-web:1
    depends_on:
      db:
        condition: service_healthy
    ports:
      - "8080:80"
      - target: 443
        published: 8443
  db:
    image: postgres:17
"#,
        )
        .expect("compose manifest");
        fs::write(
            root.path().join("openapi.yaml"),
            r"openapi: 3.1.0
info:
  title: Orders API
servers:
  - url: https://api.example.test/v1
  - url: https://owner:secret@private.example.test/v1
",
        )
        .expect("OpenAPI manifest");
        fs::write(root.path().join("vercel.json"), "{}").expect("deployment manifest");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("rediscover");
        assert_eq!(first.world, second.world);
        for (kind, label) in [
            (WorldNodeKind::Service, "orders-web"),
            (WorldNodeKind::Service, "web"),
            (WorldNodeKind::Database, "db (PostgreSQL)"),
            (WorldNodeKind::Api, "Orders API"),
            (WorldNodeKind::Domain, "https://api.example.test"),
            (WorldNodeKind::Domain, "localhost:8080"),
            (WorldNodeKind::Domain, "localhost:8443"),
            (WorldNodeKind::Deployment, "Vercel"),
            (WorldNodeKind::Runtime, "Rust"),
            (WorldNodeKind::Runtime, "Node.js"),
            (WorldNodeKind::Runtime, "Python"),
            (WorldNodeKind::Runtime, "Go"),
            (WorldNodeKind::Runtime, "OCI container"),
            (WorldNodeKind::Dependency, "serde"),
            (WorldNodeKind::Dependency, "fastify"),
            (WorldNodeKind::Dependency, "requests"),
            (WorldNodeKind::Dependency, "example.test/observability"),
        ] {
            assert!(
                first
                    .world
                    .nodes
                    .iter()
                    .any(|node| node.kind == kind && node.label == label),
                "missing {kind:?} {label}"
            );
        }
        assert!(!first.world.nodes.iter().any(|node| {
            node.label.contains("secret") || node.label.contains("private.example.test")
        }));
        let node_id = |kind, label: &str| {
            first
                .world
                .nodes
                .iter()
                .find(|node| node.kind == kind && node.label == label)
                .map(|node| node.id.clone())
                .expect("world node")
        };
        let web = node_id(WorldNodeKind::Service, "web");
        let database = node_id(WorldNodeKind::Database, "db (PostgreSQL)");
        let api = node_id(WorldNodeKind::Api, "Orders API");
        let api_domain = node_id(WorldNodeKind::Domain, "https://api.example.test");
        let deployment = node_id(WorldNodeKind::Deployment, "Vercel");
        let repository_id = "repository".to_owned();
        for (from, relation, to) in [
            (&web, "uses_database", &database),
            (&api, "served_at", &api_domain),
            (&repository_id, "deploys_to", &deployment),
        ] {
            assert!(
                first.world.relationships.iter().any(|edge| {
                    edge.from == *from && edge.relation == relation && edge.to == *to
                })
            );
        }
        assert!(first.world.nodes.iter().all(|node| !node.source.is_empty()));
        assert!(
            first
                .world
                .relationships
                .iter()
                .all(|edge| !edge.source.is_empty())
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One Kubernetes fixture proves workload, service, ingress, and secret boundaries together.
    fn kubernetes_world_model_is_bounded_routed_stable_and_secret_safe() {
        let root = tempdir().expect("root");
        fs::create_dir_all(root.path().join("k8s")).expect("Kubernetes directory");
        fs::write(
            root.path().join("k8s/orders.yaml"),
            r"apiVersion: apps/v1
kind: Deployment
metadata:
  name: orders-api
spec:
  template:
    metadata:
      labels:
        app: orders-api
        credential-token: must-not-be-retained
    spec:
      initContainers:
        - name: migrate
          image: ghcr.io/example/orders-migrate:2
      containers:
        - name: api
          image: ghcr.io/example/orders-api:7
---
apiVersion: v1
kind: Service
metadata:
  name: orders-api
spec:
  selector:
    app: orders-api
  ports:
    - name: http
      port: 8080
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: orders
spec:
  tls:
    - hosts: [orders.example.test]
  rules:
    - host: orders.example.test
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: orders-api
                port:
                  number: 8080
---
apiVersion: v1
kind: Secret
metadata:
  name: production-credentials
stringData:
  password: never-retain-this
",
        )
        .expect("Kubernetes manifest");
        fs::write(
            root.path().join("unrelated.yaml"),
            "this: [is deliberately not parsed",
        )
        .expect("unrelated YAML");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("rediscover");
        assert_eq!(first.world, second.world);
        assert!(first.services.contains(&"Kubernetes".to_owned()));
        assert!(first.watched_inputs.contains_key("k8s/orders.yaml"));
        assert!(!first.watched_inputs.contains_key("unrelated.yaml"));
        for (kind, label) in [
            (WorldNodeKind::Service, "orders-api"),
            (
                WorldNodeKind::Deployment,
                "Kubernetes Deployment default/orders-api",
            ),
            (
                WorldNodeKind::Dependency,
                "ghcr.io/example/orders-migrate:2",
            ),
            (WorldNodeKind::Dependency, "ghcr.io/example/orders-api:7"),
            (WorldNodeKind::Domain, "k8s://default/orders-api:8080"),
            (WorldNodeKind::Domain, "https://orders.example.test"),
            (WorldNodeKind::Runtime, "OCI container"),
        ] {
            assert!(
                first
                    .world
                    .nodes
                    .iter()
                    .any(|node| node.kind == kind && node.label == label),
                "missing {kind:?} {label}"
            );
        }
        assert!(first.world.nodes.iter().all(|node| {
            !node.label.contains("production-credentials")
                && !node.label.contains("never-retain-this")
                && !node.label.contains("must-not-be-retained")
        }));
        let node_id = |kind, label: &str| {
            first
                .world
                .nodes
                .iter()
                .find(|node| node.kind == kind && node.label == label)
                .map(|node| node.id.clone())
                .expect("world node")
        };
        let repository = "repository".to_owned();
        let service = node_id(WorldNodeKind::Service, "orders-api");
        let deployment = node_id(
            WorldNodeKind::Deployment,
            "Kubernetes Deployment default/orders-api",
        );
        let image = node_id(WorldNodeKind::Dependency, "ghcr.io/example/orders-api:7");
        let cluster = node_id(WorldNodeKind::Domain, "k8s://default/orders-api:8080");
        let ingress = node_id(WorldNodeKind::Domain, "https://orders.example.test");
        for (from, relation, to) in [
            (&repository, "deploys_to", &deployment),
            (&service, "deployed_as", &deployment),
            (&service, "uses_image", &image),
            (&service, "exposed_at", &cluster),
            (&ingress, "routes_to", &cluster),
        ] {
            assert!(
                first.world.relationships.iter().any(|edge| {
                    edge.from == *from && edge.relation == relation && edge.to == *to
                })
            );
        }
        assert!(first.world.nodes.iter().all(|node| !node.source.is_empty()));
        assert!(
            first
                .world
                .relationships
                .iter()
                .all(|edge| !edge.source.is_empty())
        );
    }

    #[test]
    fn helm_chart_topology_is_bounded_stable_and_ignores_sensitive_values() {
        let root = tempdir().expect("root");
        let chart_root = root.path().join("deploy/helm/storefront");
        fs::create_dir_all(&chart_root).expect("Helm chart directory");
        fs::write(
            chart_root.join("Chart.yaml"),
            r"apiVersion: v2
name: storefront
version: 1.2.3
appVersion: 7.0.0
dependencies:
  - name: postgresql
    version: 15.5.0
    repository: https://charts.example.test/private
  - name: redis
    version: 20.1.0
  - name: external-secrets
    version: 1.0.0
",
        )
        .expect("Helm chart manifest");
        fs::write(
            chart_root.join("values.yaml"),
            "databasePassword: never-retain-this\napiToken: also-secret\n",
        )
        .expect("sensitive values fixture");
        let vendored = root.path().join("vendor/charts/unrelated");
        fs::create_dir_all(&vendored).expect("unrelated chart directory");
        fs::write(
            vendored.join("Chart.yaml"),
            "apiVersion: v2\nname: unrelated\nversion: 1.0.0\n",
        )
        .expect("unrelated chart");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("rediscover");
        assert_eq!(first.world, second.world);
        assert!(first.services.contains(&"Helm".to_owned()));
        assert!(
            first
                .watched_inputs
                .contains_key("deploy/helm/storefront/Chart.yaml")
        );
        assert!(
            !first
                .watched_inputs
                .contains_key("deploy/helm/storefront/values.yaml")
        );
        assert!(
            !first
                .watched_inputs
                .contains_key("vendor/charts/unrelated/Chart.yaml")
        );

        let node_id = |kind, label: &str| {
            first
                .world
                .nodes
                .iter()
                .find(|node| node.kind == kind && node.label == label)
                .map(|node| node.id.clone())
                .expect("world node")
        };
        let chart = node_id(WorldNodeKind::Deployment, "Helm chart storefront");
        let postgresql = node_id(WorldNodeKind::Dependency, "postgresql");
        let redis = node_id(WorldNodeKind::Dependency, "redis");
        let external_secrets = node_id(WorldNodeKind::Dependency, "external-secrets");
        let database = node_id(WorldNodeKind::Database, "postgresql (PostgreSQL)");
        for (from, relation, to) in [
            ("repository", "deploys_to", chart.as_str()),
            (chart.as_str(), "depends_on", postgresql.as_str()),
            (chart.as_str(), "depends_on", redis.as_str()),
            (chart.as_str(), "depends_on", external_secrets.as_str()),
            (chart.as_str(), "uses_database", database.as_str()),
        ] {
            assert!(
                first.world.relationships.iter().any(|edge| {
                    edge.from == from && edge.relation == relation && edge.to == to
                })
            );
        }
        let retained = serde_json::to_string(&first.world).expect("serialize world model");
        for forbidden in [
            "never-retain-this",
            "also-secret",
            "charts.example.test",
            "unrelated",
        ] {
            assert!(!retained.contains(forbidden), "retained {forbidden}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture proves YAML/JSON typing, dependencies, and value exclusion together.
    fn cloudformation_topology_is_typed_dependency_aware_and_value_free() {
        let root = tempdir().expect("root");
        let cloudformation = root.path().join("infra/cloudformation");
        fs::create_dir_all(&cloudformation).expect("CloudFormation directory");
        fs::write(
            cloudformation.join("template.yaml"),
            r"AWSTemplateFormatVersion: '2010-09-09'
Resources:
  OrdersDatabase:
    Type: AWS::RDS::DBInstance
    Properties:
      MasterUserPassword: never-retain-this
  OrdersFunction:
    Type: AWS::Lambda::Function
    DependsOn: OrdersDatabase
    Properties:
      Role: !Ref ExecutionRole
      Environment:
        Variables:
          API_TOKEN: also-secret
  OrdersApi:
    Type: AWS::ApiGatewayV2::Api
    DependsOn:
      - OrdersFunction
      - MissingResource
  AssetsBucket:
    Type: AWS::S3::Bucket
  Bad-Logical-Id:
    Type: AWS::SQS::Queue
  CustomResource:
    Type: Custom::Unsafe
",
        )
        .expect("CloudFormation YAML");
        fs::write(
            cloudformation.join("cloudformation.json"),
            r#"{"Resources":{"Queue":{"Type":"AWS::SQS::Queue"},"Worker":{"Type":"AWS::Lambda::Function","DependsOn":["Queue","Queue"]}}}"#,
        )
        .expect("CloudFormation JSON");
        let unrelated = root.path().join("other");
        fs::create_dir_all(&unrelated).expect("unrelated directory");
        fs::write(
            unrelated.join("template.yaml"),
            "Resources: [this file is outside an admitted path",
        )
        .expect("unrelated template");
        fs::write(
            root.path().join("template.yaml"),
            "emailTemplate: [not-cloudformation",
        )
        .expect("ambiguous non-CloudFormation root template");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("rediscover");
        assert_eq!(first.world, second.world);
        for admitted in [
            "infra/cloudformation/template.yaml",
            "infra/cloudformation/cloudformation.json",
        ] {
            assert!(first.watched_inputs.contains_key(admitted));
        }
        assert!(!first.watched_inputs.contains_key("other/template.yaml"));
        assert!(first.watched_inputs.contains_key("template.yaml"));

        let node_id = |kind, label: &str| {
            first
                .world
                .nodes
                .iter()
                .find(|node| node.kind == kind && node.label == label)
                .map(|node| node.id.clone())
                .expect("world node")
        };
        let yaml_stack = node_id(
            WorldNodeKind::Deployment,
            "CloudFormation stack infra/cloudformation/template.yaml",
        );
        let database = node_id(
            WorldNodeKind::Database,
            "OrdersDatabase (AWS::RDS::DBInstance)",
        );
        let function = node_id(
            WorldNodeKind::Service,
            "OrdersFunction (AWS::Lambda::Function)",
        );
        let api = node_id(WorldNodeKind::Api, "OrdersApi (AWS::ApiGatewayV2::Api)");
        let bucket = node_id(WorldNodeKind::Deployment, "AssetsBucket (AWS::S3::Bucket)");
        let queue = node_id(WorldNodeKind::Deployment, "Queue (AWS::SQS::Queue)");
        let worker = node_id(WorldNodeKind::Service, "Worker (AWS::Lambda::Function)");
        for (from, relation, to) in [
            ("repository", "deploys_to", yaml_stack.as_str()),
            (yaml_stack.as_str(), "provisions", database.as_str()),
            (yaml_stack.as_str(), "provisions", function.as_str()),
            (yaml_stack.as_str(), "provisions", api.as_str()),
            (yaml_stack.as_str(), "provisions", bucket.as_str()),
            (function.as_str(), "depends_on", database.as_str()),
            (api.as_str(), "depends_on", function.as_str()),
            (worker.as_str(), "depends_on", queue.as_str()),
        ] {
            assert!(
                first.world.relationships.iter().any(|edge| {
                    edge.from == from && edge.relation == relation && edge.to == to
                })
            );
        }
        let retained = serde_json::to_string(&first.world).expect("serialize world model");
        for forbidden in [
            "never-retain-this",
            "also-secret",
            "MissingResource",
            "Bad-Logical-Id",
            "Custom::Unsafe",
            "not-cloudformation",
        ] {
            assert!(!retained.contains(forbidden), "retained {forbidden}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture proves the complete bounded .NET manifest contract.
    fn dotnet_topology_is_project_aware_dependency_ordered_and_secret_safe() {
        let root = tempdir().expect("root");
        fs::create_dir_all(root.path().join("apps/Store.Api/obj")).expect("API directory");
        fs::create_dir_all(root.path().join("packages/Store.Core")).expect("core directory");
        fs::create_dir_all(root.path().join("vendor/Unsafe")).expect("vendor directory");
        fs::write(
            root.path().join("Store.sln"),
            "Microsoft Visual Studio Solution File",
        )
        .expect("solution");
        fs::write(
            root.path().join("apps/Store.Api/Store.Api.csproj"),
            r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net9.0</TargetFramework>
    <Password>never-retain-this</Password>
    <RepositoryUrl>https://owner:secret@example.test/private</RepositoryUrl>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Dapper" Version="2.1.66" />
    <FrameworkReference Include="Microsoft.AspNetCore.App" />
    <ProjectReference Include="..\..\packages\Store.Core\Store.Core.csproj" />
    <ProjectReference Include="..\..\..\outside\Escape.csproj" />
  </ItemGroup>
</Project>"#,
        )
        .expect("API project");
        fs::write(
            root.path().join("packages/Store.Core/Store.Core.csproj"),
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Npgsql" Version="9.0.4" PrivateAssets="all" />
  </ItemGroup>
</Project>"#,
        )
        .expect("core project");
        fs::write(
            root.path().join("apps/Store.Api/obj/Generated.csproj"),
            r#"<Project><ItemGroup><PackageReference Include="must-not-appear" /></ItemGroup></Project>"#,
        )
        .expect("generated project");
        fs::write(
            root.path().join("vendor/Unsafe/Unsafe.csproj"),
            r#"<Project><ItemGroup><PackageReference Include="vendor-secret" /></ItemGroup></Project>"#,
        )
        .expect("unrelated project");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("rediscover");
        assert_eq!(first.world, second.world);
        assert!(first.build_systems.contains(&".NET SDK".to_owned()));
        assert!(first.test_systems.contains(&"dotnet test".to_owned()));
        assert!(first.package_managers.contains(&"NuGet".to_owned()));
        assert!(first.commands.iter().any(|command| {
            command.name == "build"
                && command.command == "dotnet build \"Store.sln\""
                && command.source == "Store.sln"
        }));
        assert!(first.commands.iter().any(|command| {
            command.name == "test" && command.command == "dotnet test \"Store.sln\""
        }));
        for watched in [
            "Store.sln",
            "apps/Store.Api/Store.Api.csproj",
            "packages/Store.Core/Store.Core.csproj",
        ] {
            assert!(
                first.watched_inputs.contains_key(watched),
                "missing {watched}"
            );
        }
        assert!(
            !first
                .watched_inputs
                .contains_key("vendor/Unsafe/Unsafe.csproj")
        );
        assert!(
            !first
                .watched_inputs
                .contains_key("apps/Store.Api/obj/Generated.csproj")
        );
        for (kind, label) in [
            (WorldNodeKind::Runtime, ".NET"),
            (WorldNodeKind::Service, "Store.Api"),
            (WorldNodeKind::Service, "Store.Core"),
            (WorldNodeKind::Dependency, "Dapper"),
            (WorldNodeKind::Dependency, "Microsoft.AspNetCore.App"),
            (WorldNodeKind::Dependency, "Npgsql"),
        ] {
            assert!(
                first
                    .world
                    .nodes
                    .iter()
                    .any(|node| node.kind == kind && node.label == label),
                "missing {kind:?} {label}"
            );
        }
        let retained = serde_json::to_string(&first.world).expect("serialize world model");
        for forbidden in [
            "never-retain-this",
            "owner:secret",
            "2.1.66",
            "9.0.4",
            "must-not-appear",
            "vendor-secret",
            "outside",
        ] {
            assert!(!retained.contains(forbidden), "retained {forbidden}");
        }
        let node_id = |kind, label: &str| {
            first
                .world
                .nodes
                .iter()
                .find(|node| node.kind == kind && node.label == label)
                .map(|node| node.id.clone())
                .expect("world node")
        };
        let api = node_id(WorldNodeKind::Service, "Store.Api");
        let core = node_id(WorldNodeKind::Service, "Store.Core");
        assert!(
            first.world.relationships.iter().any(|edge| {
                edge.from == api && edge.relation == "depends_on" && edge.to == core
            })
        );
        let plan = first.topology_plan();
        assert_eq!(plan.status, TopologyPlanStatus::Ready);
        let stage_of = |label: &str| {
            plan.stages
                .iter()
                .find(|stage| stage.nodes.iter().any(|node| node.label == label))
                .map(|stage| stage.index)
                .expect("planned node")
        };
        assert!(stage_of("Npgsql") < stage_of("Store.Core"));
        assert!(stage_of("Store.Core") < stage_of("Store.Api"));

        let mut watcher = ProjectWatcher::start(root.path(), project_id).expect("watch");
        fs::write(
            root.path().join("packages/Store.Core/Store.Core.csproj"),
            r#"<Project><ItemGroup><PackageReference Include="Polly" /></ItemGroup></Project>"#,
        )
        .expect("updated core project");
        let (updated, invalidation) = watcher.poll().expect("poll").expect("change");
        assert_eq!(
            invalidation.changed_inputs,
            vec!["packages/Store.Core/Store.Core.csproj"]
        );
        assert!(invalidation.invalidate_commands);
        assert!(invalidation.invalidate_world_model);
        assert!(updated.world.nodes.iter().any(|node| node.label == "Polly"));
    }

    #[test]
    fn dotnet_xml_rejects_document_types_and_malformed_projects() {
        let document_type = tempdir().expect("DOCTYPE root");
        fs::write(
            document_type.path().join("Unsafe.csproj"),
            r#"<!DOCTYPE Project [<!ENTITY secret SYSTEM "file:///secret">]><Project><ItemGroup><PackageReference Include="&secret;" /></ItemGroup></Project>"#,
        )
        .expect("DOCTYPE project");
        assert!(matches!(
            ProjectDiscoverer::discover(document_type.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Xml(_))
        ));

        let malformed = tempdir().expect("malformed root");
        fs::write(
            malformed.path().join("Broken.csproj"),
            r#"<Project><ItemGroup><PackageReference Include="Dapper"></Project>"#,
        )
        .expect("malformed project");
        assert!(matches!(
            ProjectDiscoverer::discover(malformed.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Xml(_))
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture proves providers, resources, modules, ordering, and secret exclusion together.
    fn terraform_json_topology_is_typed_dependency_aware_and_value_free() {
        let root = tempdir().expect("root");
        fs::create_dir_all(root.path().join("infra")).expect("Terraform directory");
        fs::write(
            root.path().join("infra/main.tf.json"),
            r#"{
  "terraform": {
    "required_providers": {
      "aws": {"source": "hashicorp/aws", "version": "~> 5.0"},
      "bad": {"source": "https://user:password@example.test/provider"}
    }
  },
  "module": {
    "network": {"source": "./network", "api_token": "module-secret"}
  },
  "resource": {
    "aws_db_instance": {
      "orders": {"password": "never-retain-this"}
    },
    "aws_lambda_function": {
      "worker": {
        "environment": {"variables": {"API_TOKEN": "also-secret"}},
        "depends_on": ["aws_db_instance.orders", "module.network", "module.network"]
      }
    },
    "aws_apigatewayv2_api": {
      "orders": {"depends_on": ["aws_lambda_function.worker", "missing.resource"]}
    },
    "unsafe type": {
      "ignored": {"password": "ignored-secret"}
    }
  },
  "variable": {"database_password": {"default": "variable-secret"}},
  "output": {"token": {"value": "output-secret"}}
}"#,
        )
        .expect("Terraform JSON");
        fs::create_dir_all(root.path().join("vendor")).expect("vendor directory");
        fs::write(
            root.path().join("vendor/vendor.tf.json"),
            r#"{"resource":{"aws_s3_bucket":{"unrelated":{}}}}"#,
        )
        .expect("unrelated Terraform JSON");
        fs::create_dir_all(root.path().join("infra/.terraform")).expect("cache directory");
        fs::write(
            root.path().join("infra/.terraform/cache.tf.json"),
            r#"{"resource":{"aws_s3_bucket":{"cached":{}}}}"#,
        )
        .expect("cached Terraform JSON");

        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(root.path(), project_id).expect("discover");
        let second = ProjectDiscoverer::discover(root.path(), project_id).expect("rediscover");
        assert_eq!(first.world, second.world);
        assert!(first.build_systems.contains(&"Terraform JSON".to_owned()));
        assert!(first.watched_inputs.contains_key("infra/main.tf.json"));
        assert!(!first.watched_inputs.contains_key("vendor/vendor.tf.json"));
        assert!(
            !first
                .watched_inputs
                .contains_key("infra/.terraform/cache.tf.json")
        );

        let node_id = |kind, label: &str| {
            first
                .world
                .nodes
                .iter()
                .find(|node| node.kind == kind && node.label == label)
                .map(|node| node.id.clone())
                .expect("world node")
        };
        let configuration = node_id(
            WorldNodeKind::Deployment,
            "Terraform configuration infra/main.tf.json",
        );
        let provider = node_id(
            WorldNodeKind::Dependency,
            "Terraform provider hashicorp/aws",
        );
        let network = node_id(WorldNodeKind::Deployment, "Terraform module network");
        let database = node_id(WorldNodeKind::Database, "orders (aws_db_instance)");
        let worker = node_id(WorldNodeKind::Service, "worker (aws_lambda_function)");
        let api = node_id(WorldNodeKind::Api, "orders (aws_apigatewayv2_api)");
        for (from, relation, to) in [
            ("repository", "deploys_to", configuration.as_str()),
            (configuration.as_str(), "depends_on", provider.as_str()),
            (configuration.as_str(), "provisions", network.as_str()),
            (configuration.as_str(), "provisions", database.as_str()),
            (configuration.as_str(), "provisions", worker.as_str()),
            (configuration.as_str(), "provisions", api.as_str()),
            (worker.as_str(), "uses_provider", provider.as_str()),
            (worker.as_str(), "depends_on", database.as_str()),
            (worker.as_str(), "depends_on", network.as_str()),
            (api.as_str(), "depends_on", worker.as_str()),
        ] {
            assert!(
                first.world.relationships.iter().any(|edge| {
                    edge.from == from && edge.relation == relation && edge.to == to
                }),
                "missing {from} {relation} {to}"
            );
        }
        let plan = first.topology_plan();
        assert_eq!(plan.status, TopologyPlanStatus::Ready);
        let stage_of = |id: &str| {
            plan.stages
                .iter()
                .find(|stage| stage.nodes.iter().any(|node| node.id == id))
                .map(|stage| stage.index)
                .expect("planned node")
        };
        assert!(stage_of(&provider) < stage_of(&configuration));
        assert!(stage_of(&database) < stage_of(&worker));
        assert!(stage_of(&network) < stage_of(&worker));
        assert!(stage_of(&worker) < stage_of(&api));

        let retained = serde_json::to_string(&first.world).expect("serialize world model");
        for forbidden in [
            "password@example.test",
            "module-secret",
            "never-retain-this",
            "also-secret",
            "ignored-secret",
            "variable-secret",
            "output-secret",
            "missing.resource",
            "unrelated",
            "cached",
        ] {
            assert!(!retained.contains(forbidden), "retained {forbidden}");
        }
    }

    #[test]
    fn oversized_or_malformed_topology_manifests_fail_before_unbounded_parsing() {
        let oversized = tempdir().expect("oversized root");
        fs::write(
            oversized.path().join("package.json"),
            vec![b' '; usize::try_from(MAX_MANIFEST_BYTES).expect("limit") + 1],
        )
        .expect("oversized manifest");
        assert!(matches!(
            ProjectDiscoverer::discover(oversized.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::ManifestTooLarge(_))
        ));

        let malformed = tempdir().expect("malformed root");
        fs::write(
            malformed.path().join("compose.yaml"),
            "services:\n  web: [unterminated",
        )
        .expect("malformed manifest");
        assert!(matches!(
            ProjectDiscoverer::discover(malformed.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Yaml(_))
        ));

        let malformed_kubernetes = tempdir().expect("malformed Kubernetes root");
        fs::create_dir_all(malformed_kubernetes.path().join("k8s")).expect("Kubernetes directory");
        fs::write(
            malformed_kubernetes.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\nspec: [unterminated",
        )
        .expect("malformed Kubernetes manifest");
        assert!(matches!(
            ProjectDiscoverer::discover(malformed_kubernetes.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Yaml(_))
        ));

        let malformed_helm = tempdir().expect("malformed Helm root");
        fs::write(
            malformed_helm.path().join("Chart.yaml"),
            "apiVersion: v2\nname: [unterminated",
        )
        .expect("malformed Helm chart");
        assert!(matches!(
            ProjectDiscoverer::discover(malformed_helm.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Yaml(_))
        ));

        let malformed_cloudformation = tempdir().expect("malformed CloudFormation root");
        fs::write(
            malformed_cloudformation.path().join("cloudformation.json"),
            r#"{"Resources":{"Broken": "#,
        )
        .expect("malformed CloudFormation template");
        assert!(matches!(
            ProjectDiscoverer::discover(malformed_cloudformation.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Json(_))
        ));

        let malformed_terraform = tempdir().expect("malformed Terraform root");
        fs::write(
            malformed_terraform.path().join("main.tf.json"),
            r#"{"resource":{"aws_s3_bucket": "#,
        )
        .expect("malformed Terraform JSON");
        assert!(matches!(
            ProjectDiscoverer::discover(malformed_terraform.path(), ProjectId::new()),
            Err(ProjectDiscoveryError::Json(_))
        ));
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

        fs::write(
            root.path().join("openapi.json"),
            r#"{"openapi":"3.1.0","info":{"title":"Fixture API"}}"#,
        )
        .expect("OpenAPI update");
        let (model, invalidation) = watcher.poll().expect("topology poll").expect("change");
        assert_eq!(invalidation.changed_inputs, vec!["openapi.json"]);
        assert!(invalidation.invalidate_world_model);
        assert!(!invalidation.invalidate_commands);
        assert!(
            model
                .world
                .nodes
                .iter()
                .any(|node| node.kind == WorldNodeKind::Api && node.label == "Fixture API")
        );
    }
}
