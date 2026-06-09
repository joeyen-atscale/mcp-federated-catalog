# mcp-federated-catalog

Merge `describe_model` JSON responses from N AtScale clusters into a unified
federated catalog with provenance tracking and conflict resolution.

## Usage

```
mcp-federated-catalog \
  --registry   registry.toml             \
  --catalog-dir ./catalogs/              \
  --conflict-policy priority|suffix|both \
  --format json|human                    \
  --output federated-catalog.json
```

The `--catalog-dir` must contain per-cluster JSON files named `<cluster_name>.json`
(matching cluster names in the registry).

## Conflict policies

| Policy     | Behavior                                                                 |
|------------|--------------------------------------------------------------------------|
| `priority` | Higher-priority cluster wins; loser recorded in `federation.conflicts`   |
| `suffix`   | Both kept; loser's `unique_name` gets `.<cluster_name>` appended         |
| `both`     | Both kept; both get `@<cluster_name>` suffix on `unique_name`            |

## Output schema

```json
{
  "models": [
    {
      "unique_name": "sales_model",
      "provenance": { "cluster": "prod", "model": "sales_model" },
      "measures": [
        {
          "unique_name": "Revenue",
          "provenance": { "cluster": "prod", "model": "sales_model" }
        }
      ],
      "dimensions": []
    }
  ],
  "federation": {
    "clusters": ["prod", "staging"],
    "merged_at_ms": 1749432000000,
    "conflict_policy": "priority",
    "conflicts": [
      {
        "unique_name": "Revenue",
        "model": "sales_model",
        "winner_cluster": "prod",
        "loser_cluster": "staging"
      }
    ]
  }
}
```

## Building

```bash
cargo build --release
# Binary: target/release/mcp-federated-catalog
```

## Tests

```bash
cargo test
```

20 tests covering all 6 ACs (AC1–AC6, with AC7 merged into AC5).

## Dependencies

- [`mcp-cluster-registry`](https://github.com/joeyen-atscale/mcp-cluster-registry) — cluster registry types
- `serde` / `serde_json` — serialization
- `clap` — CLI argument parsing
- `thiserror` — error types
