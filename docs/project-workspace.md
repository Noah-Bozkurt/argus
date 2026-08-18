# Project Workspace V1

Projects are the primary organizational unit in Argus. A project does not require a client and all workspace features work for personal, open-source, infrastructure, experimental and customer work.

## Project metadata

Projects now have:

- name and description
- preset: `empty`, `software`, `website`, `infrastructure`, or `client`
- status
- tags
- optional `client_id` inherited from the original model
- created/updated timestamps

The workspace API never requires or automatically creates a client relationship. The `client` preset is only a configuration hint.

## Tasks

Tasks support:

- title and description
- status: `TODO`, `IN_PROGRESS`, `BLOCKED`, `DONE`, `CANCELLED`
- priority: `LOW`, `MEDIUM`, `HIGH`, `URGENT`
- optional assignee
- optional due date
- optional milestone
- labels

V1 exposes task creation and explicit status transitions. Rich task editing, dependencies and templates can be added later without changing the core ownership model.

## Notes

Project notes are general-purpose text records with title, content, author and timestamps. V1 supports create and edit operations.

## Milestones

Milestones have a name, description, optional due date and status (`OPEN`, `COMPLETED`, `CANCELLED`). Tasks may optionally reference a milestone.

## Activity and audit

Every workspace mutation writes:

1. a security/technical `audit_events` row;
2. a human-facing `domain_events` item using the project ID as the resource ID.

The project detail view reads those project events as its Activity feed. This keeps Activity and Audit conceptually separate while allowing one mutation to feed both.

## Isolation

Every workspace query is scoped by `organization_id` and `project_id`. Milestone and assignee references are validated against the current organization/project before insertion.

## Storage boundary

The workspace implementation is isolated in `project_workspace.rs` and does not call agent/helper functionality. The current repository does not yet contain the planned Payload application layer, so V1 uses the existing PostgreSQL-backed application service rather than introducing a second uncoordinated database owner in this phase.

Before CMS/content collections are introduced, Argus should establish the planned `argus_app` / `argus_control` database ownership boundary and add Payload in a controlled migration instead of letting Payload automatically take ownership of existing control-plane tables.
