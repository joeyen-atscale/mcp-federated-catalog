//! Catalog types — mirrors the describe_model JSON schema with provenance.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Entity types (measures, dimensions)
// ---------------------------------------------------------------------------

/// Provenance block added to every entity after federation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub cluster: String,
    pub model: String,
}

/// A single measure entry in describe_model output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measure {
    pub unique_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Catch-all for additional fields (data_type, format, etc.).
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single dimension entry in describe_model output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub unique_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Catch-all for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A model block from describe_model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub unique_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(default)]
    pub measures: Vec<Measure>,
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Top-level describe_model response for one cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeModel {
    #[serde(default)]
    pub models: Vec<Model>,
    /// Catch-all for any other top-level fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ---------------------------------------------------------------------------
// Conflict record
// ---------------------------------------------------------------------------

/// Records a name conflict between two clusters' entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub unique_name: String,
    pub model: String,
    pub winner_cluster: String,
    pub loser_cluster: String,
}

// ---------------------------------------------------------------------------
// Federation metadata block
// ---------------------------------------------------------------------------

/// Top-level `federation` block written to the merged output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationBlock {
    pub clusters: Vec<String>,
    pub merged_at_ms: u64,
    pub conflict_policy: String,
    pub conflicts: Vec<ConflictRecord>,
}

// ---------------------------------------------------------------------------
// Merged output
// ---------------------------------------------------------------------------

/// The final merged catalog written to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedCatalog {
    pub models: Vec<Model>,
    pub federation: FederationBlock,
}
