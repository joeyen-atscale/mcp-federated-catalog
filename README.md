# mcp-federated-catalog

Merge the `describe_model` output of many AtScale clusters into one catalog, so an MCP client can see every model, measure, and dimension across the fleet as a single namespace — and know which cluster each one came from.

## Why it exists

An MCP client talks to one AtScale cluster at a time. Ask it "what can I query?" and you get that cluster's models — not the staging cluster's, not the cluster the finance team runs. The metadata is real and complete; it's just siloed per endpoint.

This tool collapses those silos into one document. Run `describe_model` against each cluster, drop the JSON in a directory, and the merge produces a single catalog where every entity carries a `provenance` block naming its origin cluster and model. The hard part isn't concatenation — it's what to do when two clusters define a measure with the same `unique_name`. That's a conflict, and the tool makes you pick how it resolves: keep the higher-priority cluster's, keep both under disambiguated names, or record the loser without dropping it. Nothing is silently overwritten.

## Install

Built with Cargo. The crate depends on a sibling path dependency, [`mcp-cluster-registry`](https://github.com/joeyen-atscale/mcp-cluster-registry), so clone both side by side:

```bash
git clone https://github.com/joeyen-atscale/mcp-cluster-registry
git clone https://github.com/joeyen-atscale/mcp-federated-catalog
cd mcp-federated-catalog
cargo build --release   # binary: target/release/mcp-federated-catalog
```

## Quickstart

You need two things: a cluster registry (TOML — defines each cluster's name and priority) and a directory of per-cluster `describe_model` JSON files, each named `<cluster_name>.json` to match a registry entry.

```bash
mcp-federated-catalog \
  --registry registry.toml \
  --catalog-dir ./catalogs/ \
  --conflict-policy priority \
  --format json \
  --output federated-catalog.json
```

The registry sets priority; `0` is highest. Clusters are loaded and merged in that order, so the highest-priority cluster wins ties under the `priority` policy:

```toml
[[clusters]]
name = "prod"
priority = 0
# ...

[[clusters]]
name = "staging"
priority = 1
# ...
```

A registry-listed cluster whose JSON file is missing or malformed is skipped with a warning — the merge still runs on whatever loaded. It only fails hard if *no* cluster loads. Drop `--output` to write to stdout, or pass `--format human` for a readable summary instead of JSON.

## Output

The merged JSON is a `describe_model`-shaped document plus a `federation` block. Every model and entity gains a `provenance` field; conflicts are listed explicitly.

```json
{
  "models": [
    {
      "unique_name": "sales_model",
      "provenance": { "cluster": "prod", "model": "sales_model" },
      "measures": [
        { "unique_name": "Revenue",
          "provenance": { "cluster": "prod", "model": "sales_model" } }
      ],
      "dimensions": []
    }
  ],
  "federation": {
    "clusters": ["prod", "staging"],
    "merged_at_ms": 1749432000000,
    "conflict_policy": "priority",
    "conflicts": [
      { "unique_name": "Revenue", "model": "sales_model",
        "winner_cluster": "prod", "loser_cluster": "staging" }
    ]
  }
}
```

Unknown fields on any entity (data types, formats, anything else `describe_model` emits) pass through untouched, so the merge doesn't lose detail it doesn't model.

## Conflict policies

When the same `unique_name` appears in more than one cluster for the same model, the policy decides the outcome. Every policy records the conflict in `federation.conflicts`.

| `--conflict-policy` | Behavior |
|---------------------|----------|
| `priority` (default) | Higher-priority cluster's entity is kept; the lower-priority one is dropped from `models` but recorded in `conflicts`. |
| `suffix` | Both are kept. The loser's `unique_name` gets `.<cluster_name>` appended. |
| `both` | Both are kept, both disambiguated: each gets `@<cluster_name>` appended to its `unique_name`. |

Before writing, the catalog is checked for binder compatibility — every model and entity must have a non-empty `unique_name`. Failures are reported as warnings, not fatal errors.

## Where it fits

Part of the `mcp-*` cluster-federation tools: [`mcp-cluster-registry`](https://github.com/joeyen-atscale/mcp-cluster-registry) defines the fleet and its priorities (and is this crate's one dependency), [`mcp-cross-cluster-diff`](https://github.com/joeyen-atscale/mcp-cross-cluster-diff) compares catalogs across clusters, and this crate merges them into one. Each consumes the same per-cluster `describe_model` JSON.

## Tests

```bash
cargo test
```

The suite covers the merge behaviors directly: disjoint merge with provenance, each of the three conflict policies, binder-compat validation, partial-cluster failure, and a large-input timing check.

## Status

Works and tested for the file-in, file-out merge described here. It operates on `describe_model` JSON you've already collected — it does not call clusters itself; pair it with whatever produces the per-cluster JSON. Conflicts are detected by `unique_name` within a model; the tool does not compare the two definitions, so a name that collides is treated as a conflict whether or not the underlying entities actually differ.
