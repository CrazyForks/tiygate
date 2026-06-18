-- Tune autovacuum for payload-heavy log tables.
--
-- Request payload columns are nulled after successful object-storage archival.
-- PostgreSQL will not shrink the relation file immediately, but timely VACUUM
-- marks the dead tuples / TOAST chunks reusable for future writes. The defaults
-- trigger at 20% table churn, which is too slow for this workload, so use
-- table-local reloptions rather than deployment-specific postgresql.conf flags.

ALTER TABLE IF EXISTS request_payloads SET (
    autovacuum_vacuum_scale_factor = 0.01,
    autovacuum_vacuum_threshold = 100,
    autovacuum_analyze_scale_factor = 0.05,
    autovacuum_analyze_threshold = 100,
    toast.autovacuum_vacuum_scale_factor = 0.01,
    toast.autovacuum_vacuum_threshold = 100
);

ALTER TABLE IF EXISTS request_logs SET (
    autovacuum_vacuum_scale_factor = 0.02,
    autovacuum_vacuum_threshold = 100,
    autovacuum_analyze_scale_factor = 0.05,
    autovacuum_analyze_threshold = 100,
    toast.autovacuum_vacuum_scale_factor = 0.02,
    toast.autovacuum_vacuum_threshold = 100
);
