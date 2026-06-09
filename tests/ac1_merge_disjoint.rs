//! AC1: disjoint entity sets → all entities present with correct provenance.

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
fn ac1_disjoint_measures_all_present() {
    let inputs = vec![
        ClusterCatalog {
            cluster_name: "cluster-a".to_string(),
            priority: 0,
            catalog: make_catalog(vec![make_model(
                "sales_model",
                vec![make_measure("Revenue"), make_measure("Units Sold")],
            )]),
        },
        ClusterCatalog {
            cluster_name: "cluster-b".to_string(),
            priority: 1,
            catalog: make_catalog(vec![make_model(
                "sales_model",
                vec![make_measure("Discount"), make_measure("Margin")],
            )]),
        },
    ];

    let result = merge(&inputs, ConflictPolicy::Priority);

    // One merged model.
    assert_eq!(result.models.len(), 1);
    let model = &result.models[0];

    // All four disjoint measures present.
    let names: Vec<&str> = model.measures.iter().map(|m| m.unique_name.as_str()).collect();
    assert!(names.contains(&"Revenue"), "missing Revenue; got {:?}", names);
    assert!(names.contains(&"Units Sold"), "missing Units Sold; got {:?}", names);
    assert!(names.contains(&"Discount"), "missing Discount; got {:?}", names);
    assert!(names.contains(&"Margin"), "missing Margin; got {:?}", names);

    // Provenance correctly set.
    let revenue = model.measures.iter().find(|m| m.unique_name == "Revenue").unwrap();
    assert_eq!(revenue.provenance.as_ref().unwrap().cluster, "cluster-a");

    let discount = model.measures.iter().find(|m| m.unique_name == "Discount").unwrap();
    assert_eq!(discount.provenance.as_ref().unwrap().cluster, "cluster-b");

    // No conflicts.
    assert!(result.federation.conflicts.is_empty());
}

#[test]
fn ac1_disjoint_models_all_present() {
    // Models from different clusters (no overlap) → both in output.
    let inputs = vec![
        ClusterCatalog {
            cluster_name: "cluster-a".to_string(),
            priority: 0,
            catalog: make_catalog(vec![make_model(
                "inventory_model",
                vec![make_measure("Stock")],
            )]),
        },
        ClusterCatalog {
            cluster_name: "cluster-b".to_string(),
            priority: 1,
            catalog: make_catalog(vec![make_model(
                "hr_model",
                vec![make_measure("Headcount")],
            )]),
        },
    ];

    let result = merge(&inputs, ConflictPolicy::Priority);
    assert_eq!(result.models.len(), 2);

    let model_names: Vec<&str> = result.models.iter().map(|m| m.unique_name.as_str()).collect();
    assert!(model_names.contains(&"inventory_model"));
    assert!(model_names.contains(&"hr_model"));

    assert!(result.federation.conflicts.is_empty());
    assert_eq!(result.federation.clusters, vec!["cluster-a", "cluster-b"]);
}
