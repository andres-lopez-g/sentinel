# Test and benchmark strategy

Use synthetic fixtures or controlled simulations in isolated, authorized environments. Keep Windows native collection, central ingest and ML evaluation as distinct measurements. A simulator feeding JSON cannot validate channel permission, bookmark behavior or agent CPU.

## Proposed budgets

Reference server: 4 vCPU, 8 GiB RAM, SSD, Linux containers on a single host. Reference endpoint: Windows 11 x64 VM with 2 vCPU and 4 GiB RAM. Record actual hardware, disk mode, TLS settings, commit and OS builds. Targets below are hypotheses awaiting measurement.

| Measure | Proposed gate and measurement |
| --- | --- |
| Ingestion | 50 agents × 2 events/s = 100 EPS for 30 min, then 300 EPS for 5 min. Sample 1 KiB normalized records. Count committed distinct events. |
| Latency | At 100 EPS, mean request-to-durable-receipt ≤150 ms. Source observation to search visibility p95 ≤3 s and p99 ≤8 s with synchronized clocks and a 1 s maximum batch flush interval. Measure visibility using the API independently of UI polling. |
| Endpoint cost | At 2 EPS, average service CPU ≤2% of one logical core, p95 private working set ≤64 MiB over 30 min. Record idle baseline, spool writes and peak bytes during recovery. |
| Reliability | Within healthy source and spool capacity, 100% of manifest IDs admitted before interruption appear once within retention. Report rejected, quarantined, duplicated and unavailable records separately. |
| Recovery | After 10 min disconnected at 100 aggregate EPS, drain 60,000 queued events within 10 min while continuing live intake. Measure time until backlog returns to pre-test level. |
| Database growth | Initial allowance 2 KiB/event for heap plus indexes. At sustained 100 EPS this is about 16.5 GiB/day, 115.4 GiB for seven days. Measure relation sizes before/after and plan WAL, backup and free-space headroom separately. Use a smaller retention or ingest budget if the pilot host cannot afford this. |
| Detection | Phase 3 worker sustains 200 events/s with the published enabled rule set. At 100 EPS, p95 commit-to-alert ≤5 s for immediately eligible matches. Report time-window closure delay separately. |

These budgets describe the fleet deployment. The single-PC release is governed by [LOCAL_PROFILE.md](LOCAL_PROFILE.md): the ingestion, latency and recovery rows above do not gate it, and its per-machine budget replaces the database-growth row.

## Protocol

1. Seed deterministic endpoint UUIDs, account cohorts and expected IDs. Mix successful and failed interactive/service logons, absent IPs, variable field sizes and day/night patterns. Include duplicate requests, skewed clocks and out-of-order arrivals. Generate workloads independently of server response speed and record offered versus admitted load.
2. Warm for five minutes, run steady and burst profiles, repeat three times and report all runs. Capture latency histograms, queue depth, database contention, CPU, memory, disk, query latency and recovery traffic. Report the slowest run alongside the median. Run a concurrent filtered explorer query throughout.
3. Kill the agent before/after the spool commit, interrupt the server after commit but before receipt and restart the database. Reconcile manifests and compare canonical payload hashes. Fill a small spool, expire a bookmark and remove read access in separate runs. Validate health and gap accounting.
4. Test unit mappings against sanitized XML/journal fixtures, including unsupported versions and missing fields. Run parser property tests for bounded handling of malformed data. Integration tests exercise real PostgreSQL constraints, auth, retention, revocation and pagination. End-to-end tests run the native service, server and browser on a clean lab image.
5. For rules, store expected alert and incident IDs or reproducible fingerprints, with timestamps on both sides of each window boundary. Test shared IPs, PID reuse, concurrent workers and late events. For ML, apply the chronological and cross-host protocol in the roadmap. For assistant drafts, review whether each cited record supports the specific claim and test permission filtering.
6. Publish exact seeds, input distributions, duration, dependency locks, machine configuration, raw counters and run command in the tagged benchmark report. Record every miss and explain trade-offs before revising a target. Phase 0 provides no measured results.
