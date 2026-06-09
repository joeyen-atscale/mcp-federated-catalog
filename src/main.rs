//! `mcp-federated-catalog` — CLI binary.
//!
//! Reads per-cluster describe_model JSON files from a catalog directory,
//! loads the cluster registry to determine priority order, merges the catalogs,
//! and writes a unified federated catalog JSON.

use clap::Parser;
use mcp_cluster_registry::ClusterRegistry;
use mcp_federated_catalog::{
    catalog::DescribeModel,
    merge::{merge, ClusterCatalog, ConflictPolicy},
};
use std::path::PathBuf;
use std::str::FromStr;

/// Merge describe_model JSONs from multiple AtScale clusters into a federated catalog.
#[derive(Parser, Debug)]
#[command(name = "mcp-federated-catalog", version, about)]
struct Args {
    /// Path to the cluster registry TOML file.
    #[arg(long)]
    registry: PathBuf,

    /// Directory containing per-cluster describe_model JSON files
    /// named <cluster_name>.json.
    #[arg(long)]
    catalog_dir: PathBuf,

    /// Conflict resolution policy when the same unique_name appears in multiple clusters.
    #[arg(long, default_value = "priority")]
    conflict_policy: String,

    /// Output format: json (default) or human.
    #[arg(long, default_value = "json")]
    format: String,

    /// Output file path. Defaults to stdout if not specified.
    #[arg(long)]
    output: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    // Parse conflict policy.
    let policy = ConflictPolicy::from_str(&args.conflict_policy)
        .unwrap_or_else(|e| {
            eprintln!("ERROR: {}", e);
            std::process::exit(1);
        });

    // Load registry.
    let registry_text = std::fs::read_to_string(&args.registry).unwrap_or_else(|e| {
        eprintln!("ERROR: failed to read registry {:?}: {}", args.registry, e);
        std::process::exit(1);
    });
    let registry = ClusterRegistry::from_toml(&registry_text).unwrap_or_else(|e| {
        eprintln!("ERROR: failed to parse registry: {}", e);
        std::process::exit(1);
    });

    // Load each cluster catalog in priority order.
    let clusters_by_priority = registry.by_priority();
    let mut inputs: Vec<ClusterCatalog> = Vec::new();
    let mut any_loaded = false;
    let mut all_failed = true;

    for cluster in &clusters_by_priority {
        let json_path = args.catalog_dir.join(format!("{}.json", cluster.name));
        match std::fs::read_to_string(&json_path) {
            Ok(text) => {
                match serde_json::from_str::<DescribeModel>(&text) {
                    Ok(catalog) => {
                        any_loaded = true;
                        all_failed = false;
                        inputs.push(ClusterCatalog {
                            cluster_name: cluster.name.clone(),
                            priority: cluster.priority,
                            catalog,
                        });
                    }
                    Err(e) => {
                        eprintln!(
                            "WARN: failed to parse catalog for cluster '{}': {}",
                            cluster.name, e
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "WARN: skipping cluster '{}' — catalog file {:?} not found or unreadable: {}",
                    cluster.name, json_path, e
                );
            }
        }
    }

    if !any_loaded || all_failed {
        eprintln!("ERROR: no cluster catalogs could be loaded. Exiting.");
        std::process::exit(1);
    }

    // Merge.
    let federated = merge(&inputs, policy);

    // Validate binder compat.
    if let Err(errs) = mcp_federated_catalog::merge::validate_binder_compat(&federated) {
        for e in &errs {
            eprintln!("WARN: binder-compat: {}", e);
        }
    }

    // Serialize.
    let output_text = match args.format.as_str() {
        "human" => format_human(&federated),
        _ => serde_json::to_string_pretty(&federated).expect("serialization failed"),
    };

    // Write output.
    match &args.output {
        Some(path) => {
            std::fs::write(path, &output_text).unwrap_or_else(|e| {
                eprintln!("ERROR: failed to write output to {:?}: {}", path, e);
                std::process::exit(1);
            });
            eprintln!(
                "INFO: federated catalog written to {:?} ({} models, {} conflicts)",
                path,
                federated.models.len(),
                federated.federation.conflicts.len()
            );
        }
        None => {
            println!("{}", output_text);
        }
    }
}

fn format_human(catalog: &mcp_federated_catalog::catalog::FederatedCatalog) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Federated Catalog — {} clusters, {} models, policy: {}\n",
        catalog.federation.clusters.len(),
        catalog.models.len(),
        catalog.federation.conflict_policy,
    ));
    out.push_str(&format!(
        "Clusters: {}\n",
        catalog.federation.clusters.join(", ")
    ));
    out.push_str(&format!(
        "Conflicts: {}\n\n",
        catalog.federation.conflicts.len()
    ));

    for model in &catalog.models {
        let cluster = model
            .provenance
            .as_ref()
            .map(|p| p.cluster.as_str())
            .unwrap_or("?");
        out.push_str(&format!(
            "  Model: {} (from {})\n",
            model.unique_name, cluster
        ));
        out.push_str(&format!("    Measures: {}\n", model.measures.len()));
        out.push_str(&format!("    Dimensions: {}\n", model.dimensions.len()));
    }

    if !catalog.federation.conflicts.is_empty() {
        out.push_str("\nConflicts:\n");
        for c in &catalog.federation.conflicts {
            out.push_str(&format!(
                "  {} (model: {}) — winner: {}, loser: {}\n",
                c.unique_name, c.model, c.winner_cluster, c.loser_cluster
            ));
        }
    }

    out
}
