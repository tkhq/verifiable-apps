# Turnkey Verifiable Apps

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
