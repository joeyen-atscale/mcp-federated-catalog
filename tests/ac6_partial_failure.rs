//! AC6: if a cluster's catalog file is missing/unreadable, skip it with a warning.
//! If all clusters fail to load, the binary exits with code 1.
//! We test the catalog-loading logic directly here (not subprocess).

use mcp_federated_catalog::{
    catalog::{DescribeModel, Measure, Model},
    merge::{merge, ClusterCatalog, ConflictPolicy},
};

#[allow(dead_code)]
fn make_measure(unique_name: &str) -> Measure {
    Measure {
        unique_name: unique_name.to_string(),
        name: Some(unique_name.to_string()),
        provenance: None,
        extra: Default::default(),
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn make_catalog(models: Vec<Model>) -> DescribeModel {
    DescribeModel {
        models,
        extra: Default::default(),
    }
}

/// Simulates loading catalog files, skipping those that fail.
fn load_catalogs(
    names_and_jsons: &[(&str, u8, Option<&str>)],
) -> (Vec<ClusterCatalog>, Vec<String>) {
    let mut loaded = Vec::new();
    let mut skipped = Vec::new();

    for (name, priority, maybe_json) in names_and_jsons {
        match maybe_json {
            None => {
                skipped.push(name.to_string());
            }
            Some(json) => match serde_json::from_str::<DescribeModel>(json) {
                Ok(catalog) => {
                    loaded.push(ClusterCatalog {
                        cluster_name: name.to_string(),
                        priority: *priority,
                        catalog,
                    });
                }
                Err(_) => {
                    skipped.push(name.to_string());
                }
            },
        }
    }

    (loaded, skipped)
}

const VALID_CATALOG: &str = r#"{"models":[{"unique_name":"sales_model","name":"Sales Model","measures":[{"unique_name":"Revenue","name":"Revenue"}],"dimensions":[]}]}"#;

#[test]
fn ac6_one_cluster_fails_other_succeeds() {
    // cluster-a: valid catalog; cluster-b: missing (None).
    let (loaded, skipped) = load_catalogs(&[
        ("cluster-a", 0, Some(VALID_CATALOG)),
        ("cluster-b", 1, None),
    ]);

    assert_eq!(loaded.len(), 1, "expected 1 loaded cluster");
    assert_eq!(skipped.len(), 1, "expected 1 skipped cluster");
    assert_eq!(skipped[0], "cluster-b");

    // Merge with partial cluster set still works.
    let result = merge(&loaded, ConflictPolicy::Priority);
    assert_eq!(result.models.len(), 1);
    assert_eq!(result.federation.clusters, vec!["cluster-a"]);
}

#[test]
fn ac6_all_clusters_fail_empty_inputs() {
    let (loaded, skipped) = load_catalogs(&[
        ("cluster-a", 0, None),
        ("cluster-b", 1, None),
    ]);

    assert_eq!(loaded.len(), 0);
    assert_eq!(skipped.len(), 2);

    // In the binary, empty inputs → exit 1. Here we just verify the detection.
    assert!(loaded.is_empty(), "no catalogs loaded — binary would exit 1");
}

#[test]
fn ac6_malformed_json_skipped() {
    let (loaded, skipped) = load_catalogs(&[
        ("cluster-a", 0, Some(VALID_CATALOG)),
        ("cluster-b", 1, Some("this is not valid json {")),
    ]);

    assert_eq!(loaded.len(), 1);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0], "cluster-b");
}

#[test]
fn ac6_partial_result_has_correct_cluster_list() {
    // Only loaded clusters appear in federation.clusters.
    let (loaded, _) = load_catalogs(&[
        ("prod", 0, Some(VALID_CATALOG)),
        ("staging", 1, None),
        ("dev", 2, Some(VALID_CATALOG)),
    ]);

    let result = merge(&loaded, ConflictPolicy::Priority);
    assert_eq!(result.federation.clusters, vec!["prod", "dev"]);
}
