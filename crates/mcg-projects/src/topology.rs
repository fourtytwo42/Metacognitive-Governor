use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ProjectModel, ProjectTopologyPlan, TOPOLOGY_PLAN_SCHEMA_VERSION, TopologyPlanNode,
    TopologyPlanStage, TopologyPlanStatus,
};

impl ProjectModel {
    /// Derives dependency stages only from the current immutable project snapshot.
    ///
    /// `depends_on` is the only ordering relationship. Containment and routing
    /// relationships select deployable nodes but never create invented order or
    /// execution authority. A cycle returns no executable-looking stages.
    #[must_use]
    pub fn topology_plan(&self) -> ProjectTopologyPlan {
        let nodes = self
            .world
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let provisioners = self
            .world
            .relationships
            .iter()
            .filter(|edge| edge.relation == "provisions")
            .map(|edge| edge.from.as_str())
            .collect::<BTreeSet<_>>();
        let mut selected = BTreeSet::new();
        let mut dependents = BTreeMap::<&str, BTreeSet<&str>>::new();
        let mut indegree = BTreeMap::<&str, usize>::new();

        for edge in &self.world.relationships {
            if !nodes.contains_key(edge.from.as_str()) || !nodes.contains_key(edge.to.as_str()) {
                continue;
            }
            if edge.relation == "depends_on" {
                selected.insert(edge.from.as_str());
                selected.insert(edge.to.as_str());
                if dependents
                    .entry(edge.to.as_str())
                    .or_default()
                    .insert(edge.from.as_str())
                {
                    *indegree.entry(edge.from.as_str()).or_default() += 1;
                }
            } else if edge.relation == "provisions"
                || (edge.relation == "deploys_to" && !provisioners.contains(edge.to.as_str()))
            {
                selected.insert(edge.to.as_str());
            }
        }
        for id in &selected {
            indegree.entry(id).or_default();
        }
        if selected.is_empty() {
            return empty_plan(self);
        }

        let mut remaining = selected;
        let mut stages = Vec::new();
        while !remaining.is_empty() {
            let ready = remaining
                .iter()
                .filter(|id| indegree.get(**id).copied().unwrap_or_default() == 0)
                .copied()
                .collect::<Vec<_>>();
            if ready.is_empty() {
                let unresolved_nodes = remaining
                    .iter()
                    .filter_map(|id| nodes.get(id).copied())
                    .map(TopologyPlanNode::from)
                    .collect();
                return ProjectTopologyPlan {
                    schema_version: TOPOLOGY_PLAN_SCHEMA_VERSION,
                    project_id: self.project_id,
                    project_snapshot_id: self.id,
                    status: TopologyPlanStatus::BlockedByCycle,
                    stages: Vec::new(),
                    unresolved_nodes,
                };
            }
            let stage_nodes = ready
                .iter()
                .filter_map(|id| nodes.get(id).copied())
                .map(TopologyPlanNode::from)
                .collect::<Vec<_>>();
            let Ok(index) = u16::try_from(stages.len()) else {
                return blocked_plan(self, &remaining, &nodes);
            };
            stages.push(TopologyPlanStage {
                index,
                nodes: stage_nodes,
            });
            for id in ready {
                remaining.remove(id);
                if let Some(values) = dependents.get(id) {
                    for dependent in values {
                        if let Some(value) = indegree.get_mut(dependent) {
                            *value = value.saturating_sub(1);
                        }
                    }
                }
            }
        }
        ProjectTopologyPlan {
            schema_version: TOPOLOGY_PLAN_SCHEMA_VERSION,
            project_id: self.project_id,
            project_snapshot_id: self.id,
            status: TopologyPlanStatus::Ready,
            stages,
            unresolved_nodes: Vec::new(),
        }
    }
}

fn empty_plan(project: &ProjectModel) -> ProjectTopologyPlan {
    ProjectTopologyPlan {
        schema_version: TOPOLOGY_PLAN_SCHEMA_VERSION,
        project_id: project.project_id,
        project_snapshot_id: project.id,
        status: TopologyPlanStatus::Empty,
        stages: Vec::new(),
        unresolved_nodes: Vec::new(),
    }
}

fn blocked_plan(
    project: &ProjectModel,
    remaining: &BTreeSet<&str>,
    nodes: &BTreeMap<&str, &crate::WorldNode>,
) -> ProjectTopologyPlan {
    ProjectTopologyPlan {
        schema_version: TOPOLOGY_PLAN_SCHEMA_VERSION,
        project_id: project.project_id,
        project_snapshot_id: project.id,
        status: TopologyPlanStatus::BlockedByCycle,
        stages: Vec::new(),
        unresolved_nodes: remaining
            .iter()
            .filter_map(|id| nodes.get(id).copied())
            .map(TopologyPlanNode::from)
            .collect(),
    }
}

impl From<&crate::WorldNode> for TopologyPlanNode {
    fn from(value: &crate::WorldNode) -> Self {
        Self {
            id: value.id.clone(),
            kind: value.kind,
            label: value.label.clone(),
            source: value.source.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use mcg_domain::{ProjectId, ProjectSnapshotId};

    use crate::{
        InstructionResolution, PROJECT_MODEL_SCHEMA_VERSION, ProjectModel, ProjectWorldModel,
        RepositoryModel, TopologyPlanStatus, WorldNode, WorldNodeKind, WorldRelationship,
    };

    fn project(nodes: Vec<WorldNode>, relationships: Vec<WorldRelationship>) -> ProjectModel {
        ProjectModel {
            schema_version: PROJECT_MODEL_SCHEMA_VERSION,
            id: ProjectSnapshotId::new(),
            project_id: ProjectId::new(),
            root: "fixture".into(),
            fingerprint: "f".repeat(64),
            observed_at: Utc::now(),
            repository: RepositoryModel::default(),
            languages: Vec::new(),
            build_systems: Vec::new(),
            test_systems: Vec::new(),
            package_managers: Vec::new(),
            services: Vec::new(),
            ci_systems: Vec::new(),
            important_directories: Vec::new(),
            commands: Vec::new(),
            instructions: InstructionResolution::default(),
            existing_memory: None,
            world: ProjectWorldModel {
                nodes,
                relationships,
            },
            watched_inputs: BTreeMap::new(),
        }
    }

    fn node(id: &str, kind: WorldNodeKind) -> WorldNode {
        WorldNode {
            id: id.into(),
            kind,
            label: id.into(),
            source: "fixture.yaml".into(),
        }
    }

    fn edge(from: &str, relation: &str, to: &str) -> WorldRelationship {
        WorldRelationship {
            from: from.into(),
            relation: relation.into(),
            to: to.into(),
            source: "fixture.yaml".into(),
        }
    }

    #[test]
    fn explicit_dependencies_form_stable_parallel_stages() {
        let project = project(
            vec![
                node("repository", WorldNodeKind::Repository),
                node("stack", WorldNodeKind::Deployment),
                node("database", WorldNodeKind::Database),
                node("queue", WorldNodeKind::Deployment),
                node("worker", WorldNodeKind::Service),
                node("api", WorldNodeKind::Api),
            ],
            vec![
                edge("repository", "deploys_to", "stack"),
                edge("stack", "provisions", "database"),
                edge("stack", "provisions", "queue"),
                edge("stack", "provisions", "worker"),
                edge("stack", "provisions", "api"),
                edge("worker", "depends_on", "database"),
                edge("worker", "depends_on", "queue"),
                edge("api", "depends_on", "worker"),
            ],
        );
        let first = project.topology_plan();
        let second = project.topology_plan();
        assert_eq!(first, second);
        assert_eq!(first.status, TopologyPlanStatus::Ready);
        assert_eq!(
            first
                .stages
                .iter()
                .map(|stage| stage
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            vec![vec!["database", "queue"], vec!["worker"], vec!["api"]]
        );
        assert!(first.unresolved_nodes.is_empty());
    }

    #[test]
    fn cycles_fail_closed_without_partial_stages() {
        let project = project(
            vec![
                node("a", WorldNodeKind::Service),
                node("b", WorldNodeKind::Database),
                node("independent", WorldNodeKind::Deployment),
            ],
            vec![
                edge("a", "depends_on", "b"),
                edge("b", "depends_on", "a"),
                edge("repository", "deploys_to", "independent"),
            ],
        );
        let plan = project.topology_plan();
        assert_eq!(plan.status, TopologyPlanStatus::BlockedByCycle);
        assert!(plan.stages.is_empty());
        assert_eq!(
            plan.unresolved_nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn routing_and_inventory_do_not_invent_execution_order() {
        let project = project(
            vec![
                node("repository", WorldNodeKind::Repository),
                node("domain", WorldNodeKind::Domain),
                node("service", WorldNodeKind::Service),
            ],
            vec![edge("domain", "routes_to", "service")],
        );
        let plan = project.topology_plan();
        assert_eq!(plan.status, TopologyPlanStatus::Empty);
        assert!(plan.stages.is_empty());
    }
}
