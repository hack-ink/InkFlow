# Documentation Governance

Purpose: Define how documentation is organized, updated, and kept consistent across this
repository.

## Principles

- Write documentation that is clear, concise, retrieval-friendly, and LLM-first.
- Keep contracts and invariants in `docs/spec/core/` and `docs/spec/service/`; keep runbooks and
  how-to guidance in `docs/guide/`.
- Avoid duplicating authoritative content. Link to the source of truth instead.

## Document classes and ownership

| Class | Location | Source of truth for | Update trigger |
| --- | --- | --- | --- |
| Spec | `docs/spec/core/`, `docs/spec/service/` | Contracts, schemas, pipeline behavior, invariants | Any behavior or schema change |
| Operational docs | `docs/guide/` | Runbooks, pipeline walkthroughs, maintenance | When operating procedures change |
| Analysis report | `docs/analysis/` | Evaluation outputs and review artifacts | After evaluation runs or analysis work |
| Plans | `docs/plans/` | Draft plans and design notes (non-normative) | As-needed, may drift |

## Placement rules

- If it defines a contract, it belongs in `docs/spec/core/` or `docs/spec/service/`.
- If it explains how to run or maintain a system, it belongs in `docs/guide/`.
- If it is temporary or exploratory, it belongs in `docs/plans/`.
- If it records evaluation outputs or review artifacts, it belongs in `docs/analysis/`.
- Module documentation must live under `docs/guide/` and be linked from `docs/guide/index.md`.
  Do not add module-level README files.
- Do not duplicate the same content in both spec and guide files. Spec defines what must be true;
  guide explains how to operate or implement it. When in doubt, link to the source of truth.

## Canonical entry points

- Repository overview: `README.md` (the only README in the repository).
- Specs: `docs/spec/index.md`.
- Operational docs: `docs/guide/index.md`.
- Unified documentation index: `docs/index.md`.

## Compatibility note

Legacy paths are no longer maintained. Use `docs/` paths for all references.

## Path migration map

The following path changes were applied during the documentation consolidation. Use this map to
interpret older context entries.

| Old path | New path |
| --- | --- |
| `docs/00_index.md` | `docs/index.md` |
| `docs/guide/00_index.md` | `docs/guide/index.md` |
| `docs/spec/00_index.md` | `docs/spec/index.md` |
| `docs/spec/RULES.md` | `docs/spec/core/rules.md` |
| `docs/spec/10_system_concepts_and_architecture.md` | `docs/spec/core/system_architecture.md` |
| `docs/spec/20_t0_ingestion_and_publication.md` | `docs/spec/core/t0_ingestion_publication.md` |
| `docs/spec/30_t0_refinery_pipeline.md` | `docs/spec/core/t0_refinery_pipeline.md` |
| `docs/spec/40_trace_log_standard.md` | `docs/spec/core/trace_log_standard.md` |
| `docs/spec/50_search_and_retrieval.md` | `docs/spec/core/search_and_retrieval.md` |
| `docs/spec/60_t1_entity_and_keyphrase_extraction.md` | `docs/spec/core/t1_entity_keyphrase_extraction.md` |
| `docs/spec/65_t1_quality_architecture.md` | `docs/spec/core/t1_quality_architecture.md` |
| `docs/spec/70_t1_tags.md` | `docs/spec/core/t1_tags.md` |
| `docs/guide/dev/metadata_governance.md` | `docs/spec/core/metadata_governance.md` |
| `docs/guide/dev/sql_schema_conventions.md` | `docs/guide/development/sql_schema_conventions.md` |
| `docs/guide/dev/rust_style.md` | `docs/guide/development/rust_style_guide.md` |
| `docs/guide/dev/stage_metadata_practices.md` | `docs/guide/development/stage_metadata_conventions.md` |
| `docs/guide/dev/migration_notes.md` | `docs/guide/development/migration_backlog.md` |
| `docs/guide/operation/maintenance_asset_registry.md` | `docs/guide/operations/asset_registry_maintenance.md` |
| `docs/guide/operation/maintenance_lexicon_entries.md` | `docs/guide/operations/lexicon_entries_maintenance.md` |
| `docs/guide/operation/maintenance_seed.md` | `docs/guide/operations/seed_maintenance.md` |
| `docs/guide/operation/maintenance_tags.md` | `docs/guide/operations/tag_catalog_maintenance.md` |
| `docs/guide/pipeline/enrich.md` | `docs/guide/pipelines/enrich_pipeline_overview.md` |
| `docs/guide/pipeline/chain_objects.md` | `docs/guide/pipelines/chain_object_extraction.md` |
| `docs/guide/pipeline/entityphrase.md` | `docs/guide/pipelines/entityphrase_service.md` |
| `docs/guide/test/republish_sample_run.md` | `docs/guide/testing/republish_sample_runbook.md` |

## LLM reading guidance

When answering questions about system behavior:

1. Read `AGENTS.md` for tool and scope rules.
2. Use `docs/spec/index.md` for contracts and invariants (then follow the core or service index).
3. Use `docs/guide/index.md` for runbooks and operational workflows.

## Update workflow

- Behavior or schema change: update the relevant `docs/spec/core/` or `docs/spec/service/` doc.
- Procedure change: update the relevant `docs/guide/` guide.
- Avoid copying long sections between documents. Link instead.

## Naming conventions

- Spec files use descriptive `snake_case` names with stable prefixes (`system_`, `t0_`, `t1_`,
  `trace_`, `search_`).
- Guide files use descriptive `snake_case` names within their category folders
  (`development/`, `operations/`, `pipelines/`, `testing/`).
- Plan files use `YYYY-MM-DD_<topic>_<type>.md` with `snake_case` topics (for example,
  `2026-01-01_cryptopotato_crawler_plan.md`).
