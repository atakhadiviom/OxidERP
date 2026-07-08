# Rust Frontend Plan

OxidERP will use Rust on the frontend as well as the backend.

## Decision

Use **Leptos** as the primary frontend framework.

Why Leptos:

- Rust-first frontend framework
- Supports WebAssembly/browser apps
- Component-based UI like React
- Good fit with Axum backend
- Can support SSR/hydration later
- Type-safe shared contracts with `oxiderp-sdk`

## Frontend Architecture

```text
crates/
  frontend/        # Leptos/WASM frontend app
  sdk/             # Shared Rust types used by backend and frontend
  core/            # Axum backend/API server
```

The current static HTML frontend is temporary. It will be migrated gradually into a Leptos app.

## Migration Phases

### Phase 1 — Create Rust frontend crate

- Add `crates/frontend`
- Add Leptos dependencies
- Build login page
- Build app shell/topbar/sidebar
- Reuse `oxiderp-sdk` types

### Phase 2 — Replace static dashboard

- App launcher
- Module cards
- Install/uninstall actions
- Event log

### Phase 3 — Dynamic ERP views

- List views
- Form views
- Kanban views
- Validation display
- CRUD forms

### Phase 4 — Odoo-like UX

- Search/filter/group-by
- Drag/drop kanban
- Breadcrumbs
- Action menus
- User/account menu
- Responsive/mobile layout

### Phase 5 — Production frontend build

- Compile Leptos/WASM frontend
- Serve built assets through Axum/Nginx
- Add cache busting
- Add CI build checks

## Shared Types

Frontend should use shared Rust contracts from `oxiderp-sdk` wherever possible:

- module metadata
- entity fields
- entity records
- validation errors
- login responses
- user/session data

This reduces duplicated TypeScript/JavaScript models and keeps backend/frontend aligned.

## Target Result

OxidERP becomes a full Rust stack:

```text
Rust backend: Axum + SQLx + PostgreSQL
Rust frontend: Leptos + WebAssembly
Shared contracts: oxiderp-sdk
Deployment: systemd/Nginx or Docker
```
