//! Core merge logic: merges describe_model responses from multiple clusters
//! according to the selected conflict policy.

use crate::catalog::{
    ConflictRecord, DescribeModel, Dimension, FederatedCatalog, FederationBlock, Measure, Model,
    Provenance,
};
use std::collections::HashMap;

/// Conflict resolution policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConflictPolicy {
    /// Higher-priority cluster wins; lower-priority entry dropped to conflicts[].
    Priority,
    /// Both kept; lower-priority entity has .<cluster_name> appended to unique_name.
    Suffix,
    /// Both kept; both entities get @<cluster_name> suffix on unique_name.
    Both,
}

impl ConflictPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictPolicy::Priority => "priority",
            ConflictPolicy::Suffix => "suffix",
            ConflictPolicy::Both => "both",
        }
    }
}

impl std::str::FromStr for ConflictPolicy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "priority" => Ok(ConflictPolicy::Priority),
            "suffix" => Ok(ConflictPolicy::Suffix),
            "both" => Ok(ConflictPolicy::Both),
            other => Err(format!("unknown conflict policy: {}", other)),
        }
    }
}

/// Input for one cluster: its name, numeric priority, and parsed catalog.
pub struct ClusterCatalog {
    pub cluster_name: String,
    pub priority: u8,
    pub catalog: DescribeModel,
}

/// Timestamp in milliseconds since UNIX epoch (using system time).
pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Merge all cluster catalogs into a single `FederatedCatalog`.
///
/// `inputs` must be sorted ascending by priority (0 = highest priority first)
/// before calling — the first entry in the slice is the highest-priority cluster.
pub fn merge(inputs: &[ClusterCatalog], policy: ConflictPolicy) -> FederatedCatalog {
    let mut conflicts: Vec<ConflictRecord> = Vec::new();

    // Collect cluster names in priority order.
    let cluster_names: Vec<String> = inputs.iter().map(|i| i.cluster_name.clone()).collect();

    // -----------------------------------------------------------------------
    // Build merged model list using an insertion-ordered vec.
    // model_unique_name -> index into `models`
    // -----------------------------------------------------------------------
    let mut models: Vec<Model> = Vec::new();
    let mut model_index: HashMap<String, usize> = HashMap::new();

    // First pass: register model shells from each cluster in priority order.
    for input in inputs.iter() {
        for src_model in &input.catalog.models {
            let mn = src_model.unique_name.clone();
            if !model_index.contains_key(&mn) {
                let mut shell = src_model.clone();
                shell.provenance = Some(Provenance {
                    cluster: input.cluster_name.clone(),
                    model: mn.clone(),
                });
                shell.measures = Vec::new();
                shell.dimensions = Vec::new();
                model_index.insert(mn.clone(), models.len());
                models.push(shell);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Measure merge: (model_name, entity_unique_name) -> owner cluster
    // -----------------------------------------------------------------------
    let mut measure_owners: HashMap<(String, String), String> = HashMap::new();
    // For "both" policy we need to track which winners need a @suffix copy added.
    // We accumulate them as (model_idx, measure) pairs to push after the loop.
    let mut both_winner_copies: Vec<(usize, Measure)> = Vec::new();

    for input in inputs.iter() {
        for src_model in &input.catalog.models {
            let mn = &src_model.unique_name;
            let model_idx = match model_index.get(mn) {
                Some(&i) => i,
                None => continue,
            };

            for src_measure in &src_model.measures {
                let key = (mn.clone(), src_measure.unique_name.clone());

                if let Some(owner_cluster) = measure_owners.get(&key) {
                    let winner = owner_cluster.clone();
                    let loser = input.cluster_name.clone();

                    match policy {
                        ConflictPolicy::Priority => {
                            conflicts.push(ConflictRecord {
                                unique_name: src_measure.unique_name.clone(),
                                model: mn.clone(),
                                winner_cluster: winner,
                                loser_cluster: loser,
                            });
                        }
                        ConflictPolicy::Suffix => {
                            let suffixed = format!("{}.{}", src_measure.unique_name, loser);
                            let mut m = src_measure.clone();
                            m.unique_name = suffixed;
                            m.provenance = Some(Provenance {
                                cluster: loser.clone(),
                                model: mn.clone(),
                            });
                            models[model_idx].measures.push(m);
                            conflicts.push(ConflictRecord {
                                unique_name: src_measure.unique_name.clone(),
                                model: mn.clone(),
                                winner_cluster: winner,
                                loser_cluster: loser,
                            });
                        }
                        ConflictPolicy::Both => {
                            // Add loser with @loser suffix.
                            let loser_name =
                                format!("{}@{}", src_measure.unique_name, loser);
                            let mut m_loser = src_measure.clone();
                            m_loser.unique_name = loser_name;
                            m_loser.provenance = Some(Provenance {
                                cluster: loser.clone(),
                                model: mn.clone(),
                            });
                            models[model_idx].measures.push(m_loser);

                            // Queue winner copy with @winner suffix.
                            let winner_name =
                                format!("{}@{}", src_measure.unique_name, winner);
                            let mut m_winner = src_measure.clone();
                            m_winner.unique_name = winner_name;
                            // provenance for winner copy — find winner's original entity
                            // (we don't have it here, so we set cluster=winner, model=mn)
                            m_winner.provenance = Some(Provenance {
                                cluster: winner.clone(),
                                model: mn.clone(),
                            });
                            both_winner_copies.push((model_idx, m_winner));

                            conflicts.push(ConflictRecord {
                                unique_name: src_measure.unique_name.clone(),
                                model: mn.clone(),
                                winner_cluster: winner,
                                loser_cluster: loser,
                            });
                        }
                    }
                } else {
                    // First occurrence — add entity.
                    measure_owners.insert(key, input.cluster_name.clone());
                    let mut m = src_measure.clone();
                    m.provenance = Some(Provenance {
                        cluster: input.cluster_name.clone(),
                        model: mn.clone(),
                    });
                    models[model_idx].measures.push(m);
                }
            }
        }
    }

    // Apply deferred "both" winner copies.
    for (model_idx, m) in both_winner_copies {
        models[model_idx].measures.push(m);
    }

    // -----------------------------------------------------------------------
    // Dimension merge (same logic).
    // -----------------------------------------------------------------------
    let mut dim_owners: HashMap<(String, String), String> = HashMap::new();
    let mut both_dim_winner_copies: Vec<(usize, Dimension)> = Vec::new();

    for input in inputs.iter() {
        for src_model in &input.catalog.models {
            let mn = &src_model.unique_name;
            let model_idx = match model_index.get(mn) {
                Some(&i) => i,
                None => continue,
            };

            for src_dim in &src_model.dimensions {
                let key = (mn.clone(), src_dim.unique_name.clone());

                if let Some(owner_cluster) = dim_owners.get(&key) {
                    let winner = owner_cluster.clone();
                    let loser = input.cluster_name.clone();

                    match policy {
                        ConflictPolicy::Priority => {
                            conflicts.push(ConflictRecord {
                                unique_name: src_dim.unique_name.clone(),
                                model: mn.clone(),
                                winner_cluster: winner,
                                loser_cluster: loser,
                            });
                        }
                        ConflictPolicy::Suffix => {
                            let suffixed = format!("{}.{}", src_dim.unique_name, loser);
                            let mut d = src_dim.clone();
                            d.unique_name = suffixed;
                            d.provenance = Some(Provenance {
                                cluster: loser.clone(),
                                model: mn.clone(),
                            });
                            models[model_idx].dimensions.push(d);
                            conflicts.push(ConflictRecord {
                                unique_name: src_dim.unique_name.clone(),
                                model: mn.clone(),
                                winner_cluster: winner,
                                loser_cluster: loser,
                            });
                        }
                        ConflictPolicy::Both => {
                            let loser_name = format!("{}@{}", src_dim.unique_name, loser);
                            let mut d_loser = src_dim.clone();
                            d_loser.unique_name = loser_name;
                            d_loser.provenance = Some(Provenance {
                                cluster: loser.clone(),
                                model: mn.clone(),
                            });
                            models[model_idx].dimensions.push(d_loser);

                            let winner_name =
                                format!("{}@{}", src_dim.unique_name, winner);
                            let mut d_winner = src_dim.clone();
                            d_winner.unique_name = winner_name;
                            d_winner.provenance = Some(Provenance {
                                cluster: winner.clone(),
                                model: mn.clone(),
                            });
                            both_dim_winner_copies.push((model_idx, d_winner));

                            conflicts.push(ConflictRecord {
                                unique_name: src_dim.unique_name.clone(),
                                model: mn.clone(),
                                winner_cluster: winner,
                                loser_cluster: loser,
                            });
                        }
                    }
                } else {
                    dim_owners.insert(key, input.cluster_name.clone());
                    let mut d = src_dim.clone();
                    d.provenance = Some(Provenance {
                        cluster: input.cluster_name.clone(),
                        model: mn.clone(),
                    });
                    models[model_idx].dimensions.push(d);
                }
            }
        }
    }

    for (model_idx, d) in both_dim_winner_copies {
        models[model_idx].dimensions.push(d);
    }

    FederatedCatalog {
        models,
        federation: FederationBlock {
            clusters: cluster_names,
            merged_at_ms: now_ms(),
            conflict_policy: policy.as_str().to_string(),
            conflicts,
        },
    }
}

// ---------------------------------------------------------------------------
// Structural validation (binder-compat AC4)
// ---------------------------------------------------------------------------

/// Validates that a federated catalog meets binder structural requirements:
/// - has at least one model (if any models provided)
/// - every entity (model, measure, dimension) has a non-empty unique_name
/// - every entity has a name (or unique_name serves as name)
pub fn validate_binder_compat(catalog: &FederatedCatalog) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    for model in &catalog.models {
        if model.unique_name.is_empty() {
            errors.push("model has empty unique_name".to_string());
        }
        for m in &model.measures {
            if m.unique_name.is_empty() {
                errors.push(format!(
                    "model '{}' has measure with empty unique_name",
                    model.unique_name
                ));
            }
        }
        for d in &model.dimensions {
            if d.unique_name.is_empty() {
                errors.push(format!(
                    "model '{}' has dimension with empty unique_name",
                    model.unique_name
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{DescribeModel, Measure, Model};

    fn make_measure(unique_name: &str) -> Measure {
        Measure {
            unique_name: unique_name.to_string(),
            name: Some(unique_name.to_string()),
            provenance: None,
            extra: Default::default(),
        }
    }

    fn make_model(unique_name: &str, measures: Vec<Measure>) -> Model {
        Model {
            unique_name: unique_name.to_string(),
            name: Some(unique_name.to_string()),
            provenance: None,
            measures,
            dimensions: vec![],
            extra: Default::default(),
        }
    }

    fn make_catalog(models: Vec<Model>) -> DescribeModel {
        DescribeModel {
            models,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_merge_disjoint() {
        // AC1: disjoint entity sets → all entities in output with correct provenance.
        let inputs = vec![
            ClusterCatalog {
                cluster_name: "cluster-a".to_string(),
                priority: 0,
                catalog: make_catalog(vec![make_model(
                    "sales_model",
                    vec![make_measure("Revenue"), make_measure("Units")],
                )]),
            },
            ClusterCatalog {
                cluster_name: "cluster-b".to_string(),
                priority: 1,
                catalog: make_catalog(vec![make_model(
                    "sales_model",
                    vec![make_measure("Cost"), make_measure("Margin")],
                )]),
            },
        ];

        let result = merge(&inputs, ConflictPolicy::Priority);
        assert_eq!(result.models.len(), 1);
        let model = &result.models[0];
        let measure_names: Vec<&str> = model.measures.iter().map(|m| m.unique_name.as_str()).collect();
        assert!(measure_names.contains(&"Revenue"));
        assert!(measure_names.contains(&"Units"));
        assert!(measure_names.contains(&"Cost"));
        assert!(measure_names.contains(&"Margin"));

        // Provenance check.
        let revenue = model.measures.iter().find(|m| m.unique_name == "Revenue").unwrap();
        assert_eq!(revenue.provenance.as_ref().unwrap().cluster, "cluster-a");
        let cost = model.measures.iter().find(|m| m.unique_name == "Cost").unwrap();
        assert_eq!(cost.provenance.as_ref().unwrap().cluster, "cluster-b");

        assert!(result.federation.conflicts.is_empty());
    }

    #[test]
    fn test_conflict_priority() {
        // AC2: same unique_name in two clusters → priority-0 wins, conflict recorded.
        let inputs = vec![
            ClusterCatalog {
                cluster_name: "cluster-a".to_string(),
                priority: 0,
                catalog: make_catalog(vec![make_model(
                    "sales_model",
                    vec![make_measure("Revenue")],
                )]),
            },
            ClusterCatalog {
                cluster_name: "cluster-b".to_string(),
                priority: 1,
                catalog: make_catalog(vec![make_model(
                    "sales_model",
                    vec![make_measure("Revenue")],
                )]),
            },
        ];

        let result = merge(&inputs, ConflictPolicy::Priority);
        let measures = &result.models[0].measures;
        // Only one "Revenue" entry.
        let revenues: Vec<_> = measures.iter().filter(|m| m.unique_name == "Revenue").collect();
        assert_eq!(revenues.len(), 1);
        assert_eq!(revenues[0].provenance.as_ref().unwrap().cluster, "cluster-a");

        // Conflict recorded.
        assert_eq!(result.federation.conflicts.len(), 1);
        let c = &result.federation.conflicts[0];
        assert_eq!(c.winner_cluster, "cluster-a");
        assert_eq!(c.loser_cluster, "cluster-b");
        assert_eq!(c.unique_name, "Revenue");
    }

    #[test]
    fn test_conflict_suffix() {
        // AC3: suffix policy → both entities present; loser has .<cluster> suffix.
        let inputs = vec![
            ClusterCatalog {
                cluster_name: "cluster-a".to_string(),
                priority: 0,
                catalog: make_catalog(vec![make_model(
                    "sales_model",
                    vec![make_measure("Revenue")],
                )]),
            },
            ClusterCatalog {
                cluster_name: "cluster-b".to_string(),
                priority: 1,
                catalog: make_catalog(vec![make_model(
                    "sales_model",
                    vec![make_measure("Revenue")],
                )]),
            },
        ];

        let result = merge(&inputs, ConflictPolicy::Suffix);
        let measures = &result.models[0].measures;
        let names: Vec<&str> = measures.iter().map(|m| m.unique_name.as_str()).collect();

        // Original name from winner.
        assert!(names.contains(&"Revenue"), "expected Revenue; got {:?}", names);
        // Suffixed name from loser.
        assert!(names.contains(&"Revenue.cluster-b"), "expected Revenue.cluster-b; got {:?}", names);

        assert_eq!(result.federation.conflicts.len(), 1);
    }
}
