//! AC3: conflict-policy=suffix → both entities in output; loser has .<cluster> suffix.

use mcp_federated_catalog::{
    catalog::{DescribeModel, Measure, Model},
    merge::{merge, ClusterCatalog, ConflictPolicy},
};

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
fn ac3_suffix_both_present_loser_suffixed() {
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
    let model = &result.models[0];
    let names: Vec<&str> = model.measures.iter().map(|m| m.unique_name.as_str()).collect();

    // Original name present (from winner cluster-a).
    assert!(
        names.contains(&"Revenue"),
        "expected 'Revenue' in {:?}",
        names
    );
    // Suffixed name present (from loser cluster-b).
    assert!(
        names.contains(&"Revenue.cluster-b"),
        "expected 'Revenue.cluster-b' in {:?}",
        names
    );

    // Conflict recorded.
    assert_eq!(result.federation.conflicts.len(), 1);
    let c = &result.federation.conflicts[0];
    assert_eq!(c.unique_name, "Revenue");
    assert_eq!(c.winner_cluster, "cluster-a");
    assert_eq!(c.loser_cluster, "cluster-b");
}

#[test]
fn ac3_suffix_non_conflicting_unchanged() {
    // Measures that don't conflict keep their original names.
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
                vec![make_measure("Revenue"), make_measure("Discount")],
            )]),
        },
    ];

    let result = merge(&inputs, ConflictPolicy::Suffix);
    let model = &result.models[0];
    let names: Vec<&str> = model.measures.iter().map(|m| m.unique_name.as_str()).collect();

    // Non-conflicting measures unchanged.
    assert!(names.contains(&"Units"), "missing Units; got {:?}", names);
    assert!(names.contains(&"Discount"), "missing Discount; got {:?}", names);

    // Conflicting Revenue has original + suffix.
    assert!(names.contains(&"Revenue"), "missing Revenue; got {:?}", names);
    assert!(names.contains(&"Revenue.cluster-b"), "missing Revenue.cluster-b; got {:?}", names);

    // Exactly one conflict.
    assert_eq!(result.federation.conflicts.len(), 1);
}
