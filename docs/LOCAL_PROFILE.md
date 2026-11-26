# Local profile

The first release targets one Windows machine. A native service collects the Security channel, one SQLite database holds the evidence and a browser UI bound to loopback answers the analyst's question for that host alone. There is no central server, enrollment, inter-machine transport or fleet view. This profile supersedes the server topology in [ARCHITECTURE.md](ARCHITECTURE.md), the transport routes in [API_SPECIFICATION.md](API_SPECIFICATION.md) and the central storage budget in [TESTING_STRATEGY.md](TESTING_STRATEGY.md) for this release. The fleet design remains a later gate, not a prerequisite.

## What the release drops

| Removed | Reason |
| --- | --- |
| Enrollment, tokens, mTLS, renewal, revocation | No second party to authenticate |
| Batch API, receipts, retry, quarantine | The write is local and transactional |
| Heartbeat, endpoint fleet, PostgreSQL, Compose | One host, one process tree, one file |
| Distributed event UUID, server receipt time | Channel record ID plus source generation is the identity |

Parsing, bookmarks, source health, gap accounting and retention survive unchanged.

## Requirements under this profile

| ID | Status |
| --- | --- |
| W1 | Applies. The service identity joins `Event Log Readers` and must not hold `SeSecurityPrivilege`, which would also allow clearing the log and changing audit policy. |
| W2 | Applies unchanged. |
| W3 | Not applicable: no transport. Replaced by local access control. The collector runs as a service so it observes boot and logoff events; the UI runs in the user session and reaches a loopback socket with a token stored under a restricted ACL. |
| W4 | Replaced: service restart and channel rollover, not a network outage. |
| W5 | Applies, extended with channel coverage and a bounded loss count. |
| W6 | Applies for one host. Fleet overview and cross-endpoint chains are out of scope. |
| W7 | Applies with the storage budget below. |
| W8 | Applies: install, remove and restore on a clean VM. |

## Storage model

Two tiers share one SQLite file with WAL journaling. The service filters at the source by provider and event code, so the counted records are the collected subset, not every Security record.

- `fact`: one row per admitted record. Timestamp, outcome, code, account, logon type, source address, record ID and generation. About 128 bytes with its index.
- `raw`: the rendered XML for the last 30 days, about 1.5 KiB per record after compression. The uncompressed XML is larger; the stored size is what the table counts.

Facts outlive raw by design. A daily count and an outcome survive raw expiry; the XML does not. Values below are hypotheses to replace with measurements from synthetic 4624 and 4625 fixtures.

| Profile | Collected records/day | Raw, 30 days | Facts, 12 months | Database | Install |
| --- | --- | --- | --- | --- | --- |
| Personal, one user | 300 | 14 MB | 14 MB | ~35 MB | ~75 MB |
| Typical workstation | 1,000 | 46 MB | 47 MB | ~110 MB | ~150 MB |
| Services or domain-joined | 3,000 | 138 MB | 140 MB | ~335 MB | ~375 MB |
| Build or automation host | 10,000 | 461 MB | 467 MB | ~1.1 GB | ~1.2 GB |

Database includes a 20 % allowance for page overhead and WAL. Install adds the binary, configuration and logs. Keep twice the database size free on disk, because vacuum rewrites pages.

The expected envelope is 150–400 MB per machine. Under 80 MB is unlikely and 2 GB is the ceiling for the raw year-round case. The fleet budget in [TESTING_STRATEGY.md](TESTING_STRATEGY.md) does not apply: one host carries no fleet overhead and no duplicate copies.

## Retention and deletion

Defaults are 30 days for raw and 12 months for facts, both configurable. Expired raw is deleted by day; facts are reduced to daily counts after twelve months. Run `PRAGMA incremental_vacuum` after each deletion pass so the file returns to the filesystem. Deletion is recorded in the local audit table.

## Source channel

The Windows default is 20 MB, roughly 15,000 records, which is about two days on a host that writes 7,000 records a day. Installation raises it (`wevtutil sl Security /ms:268435456`, 256 MB) and the service reports maximum size, oldest record time and coverage horizon. It marks itself degraded below 64 MB or below 24 hours of horizon. The local database is not the binding limit; the channel is.

On a stale bookmark, start a new source generation at the oldest available record, and publish `oldest_record_number − bookmarked_record_id − 1` as a lower bound on lost channel records. Eligible losses remain unknown and are labelled as such.

## Measurement plan

Measure bytes per fact and per raw record against fixtures from at least two Windows 11 builds. Gate: a clean VM with the pilot audit policy collects for 24 hours, the database stays under 50 MB, the service stays under 64 MiB private working set, and a forced channel rollover produces the expected gap count.

## Limits

- A local administrator can delete the database or the channel. Without a server there is no independent receipt; an export to external storage is the only off-host copy.
- Channel rollover while the service is stopped loses evidence permanently. The coverage horizon is the maximum supported downtime.
- One disk, no replication. Export and restore are the recovery path.
- Health is self-reported. The analyst trusts the host that produces the numbers.

## Non-goals

Fleet search, cross-endpoint correlation, high availability, hosted tenancy, anomaly scoring, the evidence assistant and the Linux collector. Each needs its own review, and the Linux daemon keeps the same local model when it arrives.
