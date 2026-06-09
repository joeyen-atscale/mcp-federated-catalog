//! AC4: merged catalog is valid describe_model JSON — structural check.
//! Every model and entity has unique_name + name fields; federation block present.

use mcp_federated_catalog::{
    catalog::{DescribeModel, Dimension, Measure, Model},
    merge::{merge, validate_binder_compat, ClusterCatalog, ConflictPolicy},
};
use serde_json::Value;

fn make_measure(unique_name: &str) -> Measure {
    Measure {
        unique_name: unique_name.to_string(),
        name: Some(unique_name.to_string()),
        provenance: None,
        extra: Default::default(),
    }
}

fn make_dim(unique_name: &str) -> Dimension {
    Dimension {
        unique_name: unique_name.to_string(),
        name: Some(unique_name.to_string()),
        provenance: None,
        extra: Default::default(),
    }
}

fn make_model(unique_name: &str, measures: Vec<Measure>, dimensions: Vec<Dimension>) -> Model {
    Model {
        unique_name: unique_name.to_string(),
        name: Some(unique_name.to_string()),
        provenance: None,
        measures,
        dimensions,
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
fn ac4_merged_json_has_models_array() {
    let inputs = vec![
        ClusterCatalog {
            cluster_name: "cluster-a".to_string(),
            priority: 0,
            catalog: make_catalog(vec![make_model(
                "sales_model",
                vec![make_measure("Revenue")],
                vec![make_dim("Date")],
            )]),
        },
        ClusterCatalog {
            cluster_name: "cluster-b".to_string(),
            priority: 1,
            catalog: make_catalog(vec![make_model(
                "hr_model",
                vec![make_measure("Headcount")],
                vec![],
            )]),
        },
    ];

    let result = merge(&inputs, ConflictPolicy::Priority);

    // Serialize and parse as generic JSON.
    let json_str = serde_json::to_string(&result).expect("serialize failed");
    let v: Value = serde_json::from_str(&json_str).expect("re-parse failed");

    // Top-level "models" array.
    assert!(v["models"].is_array(), "expected top-level 'models' array");

    // Top-level "federation" block.
    assert!(v["federation"].is_object(), "expected top-level 'federation' object");
    assert!(v["federation"]["clusters"].is_array());
    assert!(v["federation"]["conflicts"].is_array());
    assert!(v["federation"]["merged_at_ms"].is_number());
    assert!(v["federation"]["conflict_policy"].is_string());

    // Every model has unique_name.
    let models = v["models"].as_array().unwrap();
    for model in models {
        assert!(
            model["unique_name"].is_string(),
            "model missing unique_name: {:?}",
            model
        );
        // Measures.
        if let Some(measures) = model["measures"].as_array() {
            for m in measures {
                assert!(
                    m["unique_name"].is_string(),
                    "measure missing unique_name: {:?}",
                    m
                );
            }
        }
        // Dimensions.
        if let Some(dims) = model["dimensions"].as_array() {
            for d in dims {
                assert!(
                    d["unique_name"].is_string(),
                    "dimension missing unique_name: {:?}",
                    d
                );
            }
        }
    }
}

#[test]
fn ac4_validate_binder_compat_passes() {
    let inputs = vec![ClusterCatalog {
        cluster_name: "cluster-a".to_string(),
        priority: 0,
        catalog: make_catalog(vec![make_model(
            "sales_model",
            vec![make_measure("Revenue"), make_measure("Cost")],
            vec![make_dim("Date"), make_dim("Product")],
        )]),
    }];

    let result = merge(&inputs, ConflictPolicy::Priority);
    let validation = validate_binder_compat(&result);
    assert!(validation.is_ok(), "binder compat failed: {:?}", validation.err());
}

#[test]
fn ac4_provenance_block_on_every_entity() {
    let inputs = vec![
        ClusterCatalog {
            cluster_name: "prod".to_string(),
            priority: 0,
            catalog: make_catalog(vec![make_model(
                "sales_model",
                vec![make_measure("Revenue")],
                vec![make_dim("Date")],
            )]),
        },
    ];

    let result = merge(&inputs, ConflictPolicy::Priority);

    for model in &result.models {
        assert!(
            model.provenance.is_some(),
            "model '{}' missing provenance",
            model.unique_name
        );
        for m in &model.measures {
            assert!(
                m.provenance.is_some(),
                "measure '{}' missing provenance",
                m.unique_name
            );
        }
        for d in &model.dimensions {
            assert!(
                d.provenance.is_some(),
                "dimension '{}' missing provenance",
                d.unique_name
            );
        }
    }
}
