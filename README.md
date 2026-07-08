# OxidERP

OxidERP is a modular, user-friendly ERP foundation written in Rust.

It is designed like Odoo: separate business apps/modules can be enabled and managed independently while sharing one secure multi-tenant core.

## Current Prototype

This repository now contains a working Rust prototype with:

- Multi-crate Cargo workspace
- Shared SDK contract crate
- Core web server using Axum
- Odoo-style module registry
- CRM, Sales, Inventory, and Accounting module scaffolds
- Dynamic browser UI generated from module metadata
- In-memory demo tenant and entity storage
- Simple event bus log for module/entity events
- Developer CLI scaffold

## Run Locally

```bash
cargo run -p oxiderp-core
```

Open:

```text
http://localhost:8080
```

## Workspace Layout

```text
crates/
  sdk/                  Shared contracts and schemas
  core/                 Backend API and web app
  cli/                  Developer/admin CLI
  modules/
    crm/                CRM module scaffold
    sales/              Sales module scaffold
    inventory/          Inventory module scaffold
    accounting/         Accounting module scaffold
frontend/
  index.html            Dynamic user-friendly ERP UI
```

## API Endpoints

| Endpoint | Purpose |
|---|---|
| `GET /` | Web UI |
| `GET /api/health` | Health check |
| `GET /api/modules` | List enabled modules |
| `GET /api/modules/:name` | Module manifest |
| `GET /api/entities/:module/:entity` | List records |
| `POST /api/entities/:module/:entity` | Create record |
| `GET /api/events` | Event log |

## Next Engineering Milestones

1. Replace in-memory store with PostgreSQL and migrations
2. Add real tenant/user authentication
3. Add module install/uninstall screens
4. Add Wasmtime runtime for uploaded WASM modules
5. Add relational accounting tables and validation
6. Add tests and Docker deployment
