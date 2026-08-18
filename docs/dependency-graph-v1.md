# Dependency Graph & Impact V1

The dependency graph models project resources and the blast radius of a resource failure. It is project-owned and Client-independent.

## Edge direction

Every edge uses one invariant:

> source depends on target

Examples:

- `SERVICE Control API -> SERVER production-01`
- `SITE Argus -> SERVICE Control API`
- `DOMAIN app.example.com -> SITE Argus`
- `SERVICE Web -> SERVICE Control API`

Impact therefore walks edges in reverse. If `production-01` fails, Argus can derive:

```text
SERVER production-01
← SERVICE Control API
← SITE Argus
← DOMAIN app.example.com
```

## Derived edges

V1 automatically derives structural relationships already present in Argus:

- Service -> Server (`HOSTED_ON`)
- Site -> Service (`BACKED_BY`)
- Domain -> Site (`ROUTES_TO`)

These edges are read-only and have origin `DERIVED`.

## Manual edges

Operators can add project-scoped dependencies between existing resources. V1 supports:

- `DEPENDS_ON`
- `USES`

This is intended for relationships Argus cannot infer from inventory, for example:

- Web -> API
- API -> PostgreSQL service
- Worker -> Queue service

Manual edges have persistent IDs and can be deleted.

## Resource types

V1 graph nodes may be:

- SERVICE
- SITE
- DOMAIN
- SERVER
- ENVIRONMENT
- REPOSITORY

The generic dependency table uses resource type + UUID rather than six parallel edge tables.

## Referential cleanup

Because polymorphic IDs cannot use a normal foreign key to six different tables, migration `0012_dependency_graph.sql` installs delete triggers on every supported resource table. Removing a resource deletes manual dependency edges that point from or to it, preventing orphan graph records.

Derived edges need no cleanup because they are reconstructed from live inventory.

## Impact algorithm

Impact analysis is transitive and cycle-safe:

1. load the current project graph;
2. build reverse adjacency (`target -> dependent sources`);
3. breadth-first search from the selected root;
4. visit every resource at most once;
5. return distance and the full impact path for each affected resource.

A cycle therefore cannot create an infinite traversal.

## Interpretation

Impact means **could be affected because it depends on this resource**. It is not a claim that a resource caused an outage.

That distinction is important for the later Incident and Change Correlation phases: dependency paths help determine blast radius, while correlation data helps investigate possible causes.

## Audit and activity

Manual mutations emit:

- `dependency.created`
- `dependency.deleted`

Derived relationships do not generate events because they mirror existing resource assignments.

## UI

The project workspace provides:

- resource inventory for graphable project resources;
- derived and manual edge list;
- manual dependency editor;
- per-resource Impact page showing affected count, distance and full path.

## Non-goals

V1 does not implement:

- visual node-canvas rendering;
- automatic root-cause determination;
- cross-project dependencies;
- live topology discovery from network traffic;
- automatic incident creation.

## Next phase

Incidents V1 should snapshot dependency impact at incident creation. Historical incident impact must not silently change later when the dependency graph is edited.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
