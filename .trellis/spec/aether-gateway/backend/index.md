# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

This directory contains guidelines for backend development. Fill in each file with your project's specific conventions.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Desktop Gateway Lifecycle](../../aether-desktop/backend/desktop-contract.md) | Loopback listener, host EOF, bounded connection/usage drain and desktop integration | Filled |
| [Authenticated Wallet Summary](./auth-wallet-summary.md) | Authenticated wallet lookup, amounts, limits and failure behavior | Filled |
| [Dashboard Stats API](./dashboard-stats-api.md) | `/api/dashboard/stats` card assembly contract (backend ↔ frontend) | Filled |
| [Administrator JSON Backup](./admin-json-backup.md) | Version-free backup content, single-transaction restoration, cancellation and export completeness | Filled |
| [Provider Model Test API](./model-test-api.md) | Model-test diagnostic contract: on/off state bypass, key-existence precondition, status-neutral probe pattern | Filled |
| [Transport Diagnostics and SOCKS DNS](./transport-diagnostics-and-proxy.md) | Credential-safe diagnostics, preserved error context and configured proxy DNS semantics | Filled |
| [External Model Catalog Reliability](./external-model-catalog.md) | Bounded external retries, cache freshness, create-preset fallback and strict price sync | Filled |
| [Directory Structure](./directory-structure.md) | Module organization and file layout | To fill |
| [Database Guidelines](./database-guidelines.md) | ORM patterns, queries, migrations | To fill |
| [Error Handling](./error-handling.md) | Error types, handling strategies | To fill |
| [Quality Guidelines](./quality-guidelines.md) | Code standards, forbidden patterns | To fill |
| [Logging Guidelines](./logging-guidelines.md) | Structured logging, log levels | To fill |

---

## How to Fill These Guidelines

For each guideline file:

1. Document your project's **actual conventions** (not ideals)
2. Include **code examples** from your codebase
3. List **forbidden patterns** and why
4. Add **common mistakes** your team has made

The goal is to help AI assistants and new team members understand how YOUR project works.

---

**Language**: All documentation should be written in **English**.
