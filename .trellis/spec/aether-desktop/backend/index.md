# Aether Desktop Contracts

Scope: `apps/aether-desktop`, the bundled `frontend/src/desktop` entry, and the gateway lifecycle used by the macOS host.

| Specification | Use when |
| --- | --- |
| [Desktop host and gateway contract](./desktop-contract.md) | Changing IPC, child processes, credentials, packaging, or desktop validation |

User-facing installation and recovery instructions live in `docs/desktop-macos.md`. Keep Web/Docker behavior compatible; the desktop host reuses the existing gateway and management APIs.
