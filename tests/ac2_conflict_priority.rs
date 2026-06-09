//! AC2: conflict-policy=priority → higher-priority cluster wins; loser in conflicts[].

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
fn ac2_priority_winner_is_priority_zero() {
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

    let result = merge(&inputs, ConflictPolicy::Priority);
    let model = &result.models[0];

    // Revenue appears exactly once.
    let revenues: Vec<_> = model.measures.iter().filter(|m| m.unique_name == "Revenue").collect();
    assert_eq!(revenues.len(), 1, "Expected exactly one Revenue; got {:?}", revenues.len());

    // The winner is cluster-a (priority 0).
    assert_eq!(
        revenues[0].provenance.as_ref().unwrap().cluster,
        "cluster-a",
        "Winner should be cluster-a"
    );

    // Non-conflicting measures from both clusters present.
    let names: Vec<&str> = model.measures.iter().map(|m| m.unique_name.as_str()).collect();
    assert!(names.contains(&"Units"), "missing Units; got {:?}", names);
    assert!(names.contains(&"Discount"), "missing Discount; got {:?}", names);

    // One conflict recorded.
    assert_eq!(result.federation.conflicts.len(), 1);
    let conflict = &result.federation.conflicts[0];
    assert_eq!(conflict.unique_name, "Revenue");
    assert_eq!(conflict.winner_cluster, "cluster-a");
    assert_eq!(conflict.loser_cluster, "cluster-b");
    assert_eq!(conflict.model, "sales_model");
}

#[test]
fn ac2_three_cluster_priority_chain() {
    // Three clusters, same unique_name. priority=0 wins over 1 and 2.
    let inputs = vec![
        ClusterCatalog {
            cluster_name: "prod".to_string(),
            priority: 0,
            catalog: make_catalog(vec![make_model("m", vec![make_measure("Metric")])]),
        },
        ClusterCatalog {
            cluster_name: "staging".to_string(),
            priority: 1,
            catalog: make_catalog(vec![make_model("m", vec![make_measure("Metric")])]),
        },
        ClusterCatalog {
            cluster_name: "dev".to_string(),
            priority: 2,
            catalog: make_catalog(vec![make_model("m", vec![make_measure("Metric")])]),
        },
    ];

    let result = merge(&inputs, ConflictPolicy::Priority);
    let model = &result.models[0];

    let metrics: Vec<_> = model.measures.iter().filter(|m| m.unique_name == "Metric").collect();
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].provenance.as_ref().unwrap().cluster, "prod");

    // Two conflicts: staging loses, dev loses.
    assert_eq!(result.federation.conflicts.len(), 2);
    for c in &result.federation.conflicts {
        assert_eq!(c.winner_cluster, "prod");
    }
}
