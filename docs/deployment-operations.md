# Deployment and Operations

Operational reference for deploying TiyGate, configuring runtime behavior, and running it safely in production.

English | [简体中文](deployment-operations_zh.md)

## Deployment Modes

The `tiygate` binary supports three modes (selected via `--mode` / env / config):

| Mode | What it runs | When to use |
|---|---|---|
| `all` | Data plane + control plane + DB in one process | Local dev, single-node, small teams |
| `proxy` | Data plane only (stateless, horizontally scalable) | Production data plane |
| `admin` | Control plane only (Admin API + WebUI) | Production control plane |

Health probes are wired by default:

- `GET /healthz` — liveness, returns 200 even while draining (so you don't get killed mid-roll)
- `GET /readyz` — readiness, returns 503 once the pod enters draining (so the load balancer stops sending traffic)

### Admin console (WebUI)

In `all` / `admin` modes the binary serves an embedded React console at **`/admin/ui`** (e.g. `http://localhost:8080/admin/ui`). It covers the full control plane — providers, routes, API keys (with one-time secret + quota editing and live usage), the OAuth authorization-code flow, runtime settings (routing, ingress, upstream, header forwarding, payload archive, background tasks) — plus analytics: per-model / provider / API-key stats, circuit-breaker status, request-log drill-down with replay, and the audit trail. It is bilingual (English / 简体中文).

Authentication reuses the single `TIYGATE_ADMIN_TOKEN`: paste it on the login screen (validated against the Admin API, stored in the browser). The UI is compiled into the binary via `rust-embed` (the opt-in `webui` feature), so the frontend must be built before the Rust crate — run `scripts/build-with-webui.sh`, or `cd webui && npm install && npm run build` followed by `cargo build -p tiygate-server --features webui`. See `webui/README.md` for development details.

## Operations

### Graceful drain

Send `SIGTERM` (or K8s `preStop`) and the gateway:

1. Flips `/readyz` to `503` so the load balancer removes it from the pool
2. Refuses new requests with `503 + Retry-After`
3. Lets in-flight requests (including long SSE streams) finish naturally
4. On `drain_timeout` (default 30s, must be ≥ single-request `deadline`), sends a **protocol-native error frame** to any still-open streams and runs `UsageAccumulator` to prevent billing drift. The streaming path is implemented in `crates/server/src/ingress.rs::drive_upstream_stream` — it also adds a 120s idle timer (tunable via the Admin Console's Upstream settings), an opt-in total wall-clock budget (default disabled), and a 30s SSE keepalive (`SseKeepaliveStream`) so middleboxes do not silently drop long-quiet streams
5. Flushes the telemetry channel, releases resources, exits

### Configuration

TiyGate configuration is split into two layers:

**1. Startup-only environment variables** — read once at process start, require a restart to change:

| Variable | Default | Purpose |
| --- | --- | --- |
| `TIYGATE_LISTEN_ADDR` | `0.0.0.0:3000` | Listen address for the HTTP server. |
| `TIYGATE_MODE` | `all` | Deployment mode. `all` (data + control in one process), `proxy` (data plane only), `admin` (control plane only). |
| `TIYGATE_DATABASE_URL` | unset | Database connection string (SQLite or Postgres). When unset, the server falls back to a legacy in-memory config store with no Admin API. |
| `TIYGATE_ADMIN_TOKEN` | unset | Bearer token required by the Admin API. When unset, Admin API requests are rejected. |
| `TIYGATE_MASTER_KEY` | unset | AES-256-GCM master key used to encrypt provider keys, OAuth tokens, and S3 credentials at rest. Accepts 64 hex chars or standard base64. When unset, secrets are stored in cleartext (the server logs a warning; acceptable for local dev only). |
| `TIYGATE_REDIS_URL` | unset | When set (and built with the `redis-quota` feature), quota counters are shared across replicas via Redis instead of per-replica in-memory. |
| `RUST_LOG` | `info` | `tracing` / `tracing-subscriber` filter. Examples: `info`, `tiygate=debug`, `tiygate_server::ingress=trace`. |

**2. Runtime-tunable settings** — managed through the Admin Console at **`/admin/ui/settings`** (backed by the `settings` table, exposed via `GET/PUT /admin/v1/settings`). These are hot-reloaded: the data plane polls for changes and atomically switches to the new snapshot without a restart.

On first start, the env values below are seeded into the `settings` table as initial defaults; after that, **the settings table is the single source of truth** — editing `.env` again has no effect unless the `settings` table is cleared.

The Settings page is organized into five cards:

| Card | What it controls | Seeded from env |
| --- | --- | --- |
| **Routing & Ingress** | Default routing strategy, max body bytes, max in-flight, max queue depth, acquire timeout, raw-envelope capture media types | `TIYGATE_ROUTING_STRATEGY`, `TIYGATE_MAX_BODY_BYTES`, `TIYGATE_MAX_INFLIGHT`, `TIYGATE_RAW_ENVELOPE_CAPTURE_MEDIA` |
| **Upstream** | Stream idle / total timeouts, TCP keepalive, pool idle timeout, TCP nodelay | `TIYGATE_UPSTREAM_STREAM_IDLE_TIMEOUT_SECS`, `TIYGATE_UPSTREAM_STREAM_TOTAL_TIMEOUT_SECS`, `TIYGATE_UPSTREAM_TCP_KEEPALIVE_SECS`, `TIYGATE_UPSTREAM_POOL_IDLE_TIMEOUT_SECS`, `TIYGATE_UPSTREAM_TCP_NODELAY` |
| **Header Forwarding** | Request / response header deny-lists (comma-separated) | `TIYGATE_FORWARD_REQUEST_HEADER_DENY`, `TIYGATE_FORWARD_RESPONSE_HEADER_DENY` |
| **Payload Archive** | S3-compatible object-storage archiving of full request/response payloads (enabled flag, endpoint, region, bucket, credentials, prefix, force-path-style, scan interval, batch size, concurrency, timeout, max retries) | `TIYGATE_PAYLOAD_ARCHIVE_*` family |
| **Background Tasks** | Log retention interval & days, epoch poll interval, token-stats interval & lookback days | `TIYGATE_LOG_RETENTION_*`, `TIYGATE_EPOCH_POLL_INTERVAL_SECS`, `TIYGATE_TOKEN_STATS_*` |

- **Epoch versioning**: the data plane polls for config changes and atomically switches to the new snapshot; in-flight requests keep the old epoch until they finish — no half-old, half-new state mid-request.
- **Secret encryption**: provider keys / OAuth tokens / encrypted S3 settings are AES-GCM encrypted at rest using `TIYGATE_MASTER_KEY`. Encrypted settings are redacted on `GET /admin/v1/settings`.

### Caching

Only **embedding** requests are cached. LLM chat/completion is **not** cached — by design (non-determinism makes response caching value-low and risk-high). The cache is pluggable: process-local LRU by default, Redis shared backend for multi-replica deployments.

### Payload archive to S3

When enabled, a background worker gzip-compresses the full request/response payload detail of each request (8 objects per request — raw body + parsed metadata for each of the 4 hops: client→gateway, gateway→provider, provider→gateway, gateway→client), uploads them to S3-compatible object storage, verifies sha256/size, and then clears the payload text from the database in the same transaction. This keeps the DB lean for high-volume deployments while preserving full replay fidelity.

The Admin Console's request replay feature transparently hydrates archived objects back from S3 on demand (verify → decompress → return), so the user experience is unchanged whether a request's payloads live in the DB or in object storage.

Object lifecycle is decoupled from DB retention — the worker never deletes from S3; use bucket lifecycle policies for expiry.

Enable and configure payload archiving in the Admin Console under **Settings → Payload Archive**. The env variables (`TIYGATE_PAYLOAD_ARCHIVE_*`) only seed the initial defaults on first start; after that the settings table is authoritative and changes apply without a restart. See `.env.example` for the full variable list.

### Distributed tracing

W3C `traceparent` / `tracestate` are extracted from the inbound request and re-injected on the upstream call. The gateway span attaches to the caller's trace as a parent. Logs and traces are cross-linkable by `trace_id`.
