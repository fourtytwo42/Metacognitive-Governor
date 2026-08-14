use serde::{Deserialize, Serialize};

use crate::{ModelDescriptor, ModelPrice, PriceProvenance, ProviderError, Usage};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderModelRef {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteCandidate {
    pub target: ProviderModelRef,
    pub model: ModelDescriptor,
    #[serde(default)]
    pub preference: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredCapability {
    Tools,
    StructuredOutput,
    Vision,
    Reasoning,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingRequirements {
    pub capabilities: Vec<RequiredCapability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RoutingPolicy {
    pub allow_unknown_cost: bool,
    pub max_estimated_cost_usd: Option<f64>,
    pub max_fallbacks: u32,
    pub shadow_enabled: bool,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            allow_unknown_cost: true,
            max_estimated_cost_usd: None,
            max_fallbacks: 2,
            shadow_enabled: cfg!(debug_assertions),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    pub amount_usd: Option<f64>,
    pub provenance: PriceProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedRoute {
    pub target: ProviderModelRef,
    pub estimated_cost: CostEstimate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutePlan {
    pub pinned: PlannedRoute,
    pub fallbacks: Vec<PlannedRoute>,
    pub shadow_allowed: bool,
}

/// Produces a deterministic, per-request pinned route and bounded fallback list.
///
/// Unknown capability values do not satisfy a required capability. Unknown costs
/// are retained only when policy explicitly allows them.
///
/// # Errors
/// Returns [`ProviderError::Configuration`] if no candidate satisfies capability
/// and cost constraints or token estimates are invalid.
pub fn plan_routes(
    candidates: &[RouteCandidate],
    requirements: &RoutingRequirements,
    policy: RoutingPolicy,
    estimated_input_tokens: u64,
    estimated_output_tokens: u64,
) -> Result<RoutePlan, ProviderError> {
    if policy
        .max_estimated_cost_usd
        .is_some_and(|limit| !limit.is_finite() || limit < 0.0)
    {
        return Err(ProviderError::Configuration(
            "maximum estimated cost must be finite and non-negative".into(),
        ));
    }
    let mut routes = candidates
        .iter()
        .filter(|candidate| supports(&candidate.model, requirements))
        .filter_map(|candidate| {
            let cost = estimate_cost(
                candidate.model.price.as_ref(),
                estimated_input_tokens,
                estimated_output_tokens,
            );
            if cost.amount_usd.is_none() && !policy.allow_unknown_cost {
                return None;
            }
            if let (Some(limit), Some(amount)) = (policy.max_estimated_cost_usd, cost.amount_usd)
                && amount > limit
            {
                return None;
            }
            Some((
                candidate.preference,
                PlannedRoute {
                    target: candidate.target.clone(),
                    estimated_cost: cost,
                },
            ))
        })
        .collect::<Vec<_>>();
    routes.sort_by(|(left_preference, left), (right_preference, right)| {
        left_preference
            .cmp(right_preference)
            .then_with(|| compare_cost(&left.estimated_cost, &right.estimated_cost))
            .then_with(|| left.target.provider_id.cmp(&right.target.provider_id))
            .then_with(|| left.target.model_id.cmp(&right.target.model_id))
    });
    let mut routes = routes.into_iter().map(|(_, route)| route);
    let pinned = routes.next().ok_or_else(|| {
        ProviderError::Configuration(
            "no provider model satisfies the routing requirements and budget".into(),
        )
    })?;
    let fallbacks = routes.take(policy.max_fallbacks as usize).collect();
    Ok(RoutePlan {
        pinned,
        fallbacks,
        shadow_allowed: policy.shadow_enabled,
    })
}

#[must_use]
pub fn actual_cost(price: Option<&ModelPrice>, usage: &Usage) -> CostEstimate {
    let Some(price) = price else {
        return CostEstimate {
            amount_usd: None,
            provenance: PriceProvenance::Unknown,
        };
    };
    let uncached = usage.input_tokens.saturating_sub(usage.cached_input_tokens);
    let components = [
        price.input_per_million.map(|rate| (uncached, rate)),
        price
            .cached_input_per_million
            .map(|rate| (usage.cached_input_tokens, rate)),
        price
            .output_per_million
            .map(|rate| (usage.output_tokens, rate)),
    ];
    let amount_usd = components.into_iter().try_fold(0.0, |total, component| {
        let (tokens, rate) = component?;
        let tokens = u32::try_from(tokens).ok()?;
        Some(total + f64::from(tokens) * rate / 1_000_000.0)
    });
    CostEstimate {
        amount_usd,
        provenance: price.provenance.clone(),
    }
}

#[must_use]
pub const fn should_fallback(error: &ProviderError) -> bool {
    error.is_retriable()
}

fn estimate_cost(
    price: Option<&ModelPrice>,
    input_tokens: u64,
    output_tokens: u64,
) -> CostEstimate {
    actual_cost(
        price,
        &Usage {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens.saturating_add(output_tokens),
            ..Usage::default()
        },
    )
}

fn supports(model: &ModelDescriptor, requirements: &RoutingRequirements) -> bool {
    requirements.capabilities.iter().all(|capability| {
        (match capability {
            RequiredCapability::Tools => model.capabilities.tools,
            RequiredCapability::StructuredOutput => model.capabilities.structured_output,
            RequiredCapability::Vision => model.capabilities.vision,
            RequiredCapability::Reasoning => model.capabilities.reasoning,
        }) == Some(true)
    })
}

fn compare_cost(left: &CostEstimate, right: &CostEstimate) -> std::cmp::Ordering {
    match (left.amount_usd, right.amount_usd) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{ModelCapabilities, ProviderError};

    fn candidate(provider: &str, tools: Option<bool>, rate: Option<f64>) -> RouteCandidate {
        RouteCandidate {
            target: ProviderModelRef {
                provider_id: provider.into(),
                model_id: "model".into(),
            },
            model: ModelDescriptor {
                id: "model".into(),
                display_name: "Model".into(),
                owned_by: None,
                context_window: None,
                capabilities: ModelCapabilities {
                    tools,
                    ..ModelCapabilities::default()
                },
                price: rate.map(|rate| ModelPrice {
                    input_per_million: Some(rate),
                    cached_input_per_million: Some(rate / 2.0),
                    output_per_million: Some(rate * 2.0),
                    currency: "USD".into(),
                    provenance: PriceProvenance::ProviderReported,
                }),
                metadata: Value::Null,
            },
            preference: 0,
        }
    }

    #[test]
    fn pins_cheapest_qualified_route_and_bounds_fallbacks() {
        let plan = plan_routes(
            &[
                candidate("unknown", true.into(), None),
                candidate("expensive", true.into(), Some(10.0)),
                candidate("cheap", true.into(), Some(1.0)),
                candidate("incapable", false.into(), Some(0.1)),
            ],
            &RoutingRequirements {
                capabilities: vec![RequiredCapability::Tools],
            },
            RoutingPolicy {
                max_fallbacks: 1,
                ..RoutingPolicy::default()
            },
            1_000,
            100,
        )
        .expect("route plan");
        assert_eq!(plan.pinned.target.provider_id, "cheap");
        assert_eq!(plan.fallbacks.len(), 1);
        assert_eq!(plan.fallbacks[0].target.provider_id, "expensive");
    }

    #[test]
    fn actual_cost_separates_cached_input() {
        let model = candidate("priced", Some(true), Some(2.0)).model;
        let cost = actual_cost(
            model.price.as_ref(),
            &Usage {
                input_tokens: 1_000,
                cached_input_tokens: 400,
                output_tokens: 100,
                total_tokens: 1_100,
                reasoning_tokens: 0,
            },
        );
        assert_eq!(cost.amount_usd, Some(0.002));
    }

    #[test]
    fn authentication_failure_never_triggers_billing_fallback() {
        assert!(!should_fallback(&ProviderError::Authentication(
            "expired".into()
        )));
        assert!(should_fallback(&ProviderError::Http {
            status: 503,
            message: "unavailable".into(),
            retriable: true,
        }));
    }
}
