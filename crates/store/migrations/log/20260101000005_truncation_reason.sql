-- Gateway-side stream truncation reason for the Provider -> Gateway
-- streaming (SSE) response. When a streaming response ends before a
-- clean natural completion, the gateway records why: "idle" (idle
-- timer fired), "total" (total wall-clock budget elapsed),
-- "upstream_error" (upstream connection errored mid-stream), or
-- "client_disconnect" (downstream client cancelled).
--
-- Populated by the OLTP sink on the telemetry background task from
-- `ExchangeCapture::truncation_reason`. NULL for a clean end-of-stream,
-- for non-stream exchanges, and for rows captured before this column
-- existed. Note: request_logs.status / http_status keep HTTP semantics
-- (a mid-stream truncation still has http_status 200), so this column
-- is the authoritative signal for "status=ok but actually truncated".

ALTER TABLE request_payloads ADD COLUMN truncation_reason TEXT;
