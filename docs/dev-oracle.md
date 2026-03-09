# Local Oracle (Developer) Quickstart

This guide shows how to run a local Oracle Database Free instance for development parity with production.

The checked-in setup uses the `latest-lite` Oracle Free image because it starts faster and is more reliable on local Docker runtimes while preserving Oracle SQL compatibility for this application.

1. If you use Colima on macOS, start it with enough resources for Oracle Free:

```bash
colima start --vm-type=vz --mount-type=virtiofs --cpu 2 --memory 3
```

2. Start the local Oracle container:

```bash
docker compose up --build
```

This starts `oracle-dev`, waits for the database healthcheck, runs the `migrate` service once, and then starts the app.

The healthcheck connects to `FREEPDB1` from inside the Oracle container with `sqlplus`, which avoids depending on host-side variable interpolation and catches the common case where the listener is up but the PDB service is not yet accepting connections.

3. Wait for the container to become healthy (may take several minutes). Check logs:

```bash
docker compose logs -f oracle-dev
```

Dev-only Oracle bootstrap note

- The local dev container mounts `scripts/oracle-dev-startup/01-grant-pdbadmin.sql` into Oracle's startup hook directory.
- That SQL runs only for the local Oracle Free container and grants `RESOURCE` and `UNLIMITED TABLESPACE` to `pdbadmin` inside `FREEPDB1`.
- This is intentionally development-only bootstrap behavior so the checked-in schema can be created automatically on a fresh local volume.
- Production does not use this path; production schema changes still come from the migration binary against the deployed image and wallet-based Oracle connection.

4. Set `.env` values (example):

```
ORACLE_PDB_USER=pdbadmin
ORACLE_PWD=ChangeMe123
ORACLE_PDB_CONNECT_STRING=//localhost:1521/FREEPDB1
```

5. If you want to run the migration binary directly from the host instead of Compose:

```bash
RUST_ENV=development cargo run --bin migrate
```

6. If you want to run the app directly from the host instead of Compose:

```bash
RUST_ENV=development cargo run
```

Notes:
- The Oracle Free image requires notable disk and memory; only opt-in developers should run it locally.
- CI should use an Oracle-backed integration stage if you want full SQL parity with development and production.
- Oracle's published Free edition limits are up to `2` CPU threads, `2 GiB` of database RAM, and `12 GiB` of user data. Those are product limits, not a guarantee that a `2 GiB` Colima VM leaves enough headroom for Linux, Docker, and Oracle startup overhead.
- On Colima, the default `2 CPU / 2 GiB` VM was too tight on this machine. The smallest profile validated here is `colima start --vm-type=vz --mount-type=virtiofs --cpu 2 --memory 3`.
- On macOS, the app now initializes the Oracle client explicitly from `OCI_LIB_DIR`, so host-side `cargo run` and `cargo test` no longer need a wrapper script.
- The development runtime now accepts either `DEV_ORACLE_*` or `ORACLE_PDB_*` environment variables. The default Compose stack uses `ORACLE_PDB_*`.

Validated macOS Colima workaround

- With Colima at its default `2 CPU / 2 GiB` VM size, the entire Linux VM only had about `2.05 GB` total memory. That budget had to cover the kernel, dockerd, filesystem cache, and the Oracle container, so the database was effectively below Oracle's usable headroom even though the VM nominally matched the published floor.
- In that state, Oracle Free started the container process but the database aborted during initialization and the listener never registered `FREEPDB1`.
- Increasing Colima to `2` CPUs and `3` GiB memory was enough to fix the Oracle startup failure on this machine.
- After that resize, `oracle-dev` reached a healthy state, `FREEPDB1` opened successfully, and the migration binary connected after the process initialized Oracle from `OCI_LIB_DIR`.
- The remaining local bootstrap requirement is Oracle Instant Client on macOS. Set `OCI_LIB_DIR` to the client library directory if you do not use the default path from your local `.env`.

Testing guidance

- Run migrations after the container is healthy:

```bash
RUST_ENV=development cargo run --bin migrate
```

- To run integration tests against the local Oracle instance (developer machine only):

```bash
RUST_ENV=development \
	cargo test --test integration_receipts_audit -- --nocapture
```

CI considerations

- CI environments often cannot run Oracle container images due to resource and licensing constraints. If you want true SQL parity, configure a dedicated CI job or runner that can pull the Oracle image and run the Oracle-backed integration tests.
- If you want full parity in CI, ensure the runner has sufficient disk/memory and that you have permission to pull the Oracle image in your CI environment.
