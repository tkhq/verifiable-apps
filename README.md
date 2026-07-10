# Turnkey Verifiable Apps

## :warning: Archived & Deprecated

This repository is deprecated and has been archived.

- No new features, bug fixes, or security updates
- Existing code is kept for historical and reference purposes only.
- Please do not use this repository for new production workloads.

## Repository Layout

```
verifiable-apps/
     Cargo.toml              # Workspace configuration
     common/                 # Shared utilities for apps
         host_primitives/    # Core primitives for host operations  
         health_check/       # Kubernetes-friendly gRPC health check service implementation
     codegen/                # Build script for generating types from protobufs
```

## Development

Generate code from protobuf type definitions:

```sh
make codegen
```
