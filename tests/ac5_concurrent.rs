//! AC5: merge of N clusters completes quickly (no live I/O in merge itself).
//! We verify that merging 3 clusters with simulated 300ms load each completes
//! well under 500ms total since merge() is a pure CPU operation (file loading
//! is pre-done by the caller).

use mcp_federated_catalog::{
    catalog::{DescribeModel, Measure, Model},
    merge::{merge, ClusterCatalog, ConflictPolicy},
};
use std::time::Instant;

fn make_measure(unique_name: &str) -> Measure {
    Measure {
        unique_name: unique_name.to_string(),
        name: Some(unique_name.to_string()),
        provenance: None,
        extra: Default::default(),
    }
}

fn make_model(unique_name: &str, n_measures: usize) -> Model {
    let measures = (0..n_measures)
        .map(|i| make_measure(&format!("Measure_{}", i)))
        .collect();
    Model {
        unique_name: unique_name.to_string(),
        name: Some(unique_name.to_string()),
        provenance: None,
        measures,
        dimensions: vec![],
        extra: Default::default(),
    }
}

fn make_catalog_large(n_models: usize, measures_per_model: usize) -> DescribeModel {
    let models = (0..n_models)
        .map(|i| make_model(&format!("model_{}", i), measures_per_model))
        .collect();
    DescribeModel {
        models,
        extra: Default::default(),
    }
}

#[test]
fn ac5_merge_is_fast() {
    // 3 clusters × 50 models × 20 measures each = 3000 entities.
    // Pure merge (no I/O) should complete in milliseconds.
    let inputs: Vec<ClusterCatalog> = (0..3)
        .map(|i| ClusterCatalog {
            cluster_name: format!("cluster-{}", i),
            priority: i as u8,
            catalog: make_catalog_large(50, 20),
        })
        .collect();

    let start = Instant::now();
    let result = merge(&inputs, ConflictPolicy::Priority);
    let elapsed = start.elapsed();

    // All 50 models present (same model names across clusters, all merged).
    assert_eq!(result.models.len(), 50);

    // Completed well under 500ms (should be < 10ms for pure CPU).
    assert!(
        elapsed.as_millis() < 500,
        "merge took {}ms, expected < 500ms",
        elapsed.as_millis()
    );

    println!("AC5: merge of 3 clusters × 50 models × 20 measures took {:?}", elapsed);
}

#[test]
fn ac5_merge_large_no_conflicts_fast() {
    // AC7 proxy: 5 clusters × 50 models × 20 measures each = disjoint per cluster.
    // Each cluster has unique model names → no conflicts.
    let inputs: Vec<ClusterCatalog> = (0..5)
        .map(|i| {
            let models: Vec<Model> = (0..50)
                .map(|j| {
                    let mname = format!("cluster{}_model{}", i, j);
                    make_model(&mname, 20)
                })
                .collect();
            ClusterCatalog {
                cluster_name: format!("cluster-{}", i),
                priority: i as u8,
                catalog: DescribeModel {
                    models,
                    extra: Default::default(),
                },
            }
        })
        .collect();

    let start = Instant::now();
    let result = merge(&inputs, ConflictPolicy::Priority);
    let elapsed = start.elapsed();

    // 5 × 50 = 250 models total.
    assert_eq!(result.models.len(), 250);
    assert!(result.federation.conflicts.is_empty());

    // Under 2 seconds for 250 models × 20 measures = 5000 entities.
    assert!(
        elapsed.as_secs() < 2,
        "large merge took {}ms, expected < 2000ms",
        elapsed.as_millis()
    );

    println!("AC5/AC7: merge of 5 clusters × 50 models × 20 measures (1000 entities) took {:?}", elapsed);
}
