# OxidERP Architecture

## Design Goals

- User-friendly ERP UI
- Odoo-style separated business modules
- Rust backend and shared contracts
- Future WebAssembly module runtime
- Multi-tenant SaaS-ready architecture

## Module Model

Each module declares:

- Name, title, icon, version, category
- Permissions
- Entities and fields
- Views
- Actions
- Published/subscribed events

The core server renders these manifests into API responses. The frontend dynamically builds cards, lists, and forms from the same metadata.

## Current Modules

| Module | Purpose |
|---|---|
| CRM | Leads and opportunities |
| Sales | Orders and sales workflow |
| Inventory | Products and stock |
| Accounting | Journals and bookkeeping foundation |

## Storage

The prototype uses an in-memory store for fast iteration. Production storage should use PostgreSQL with tenant-scoped tables and JSONB entity records for module-defined fields.

## WASM Runtime Roadmap

Server-side modules will later compile to `wasm32-wasip1` and run inside Wasmtime with:

- Timeouts
- Memory limits
- Fuel/instruction limits
- Strict host API permissions
- No direct database access
