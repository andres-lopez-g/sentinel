# Agent operations · current increment

This describes what the code in this repository does today, not the final Windows service design. It is deliberately narrower than [LOCAL_PROFILE.md](LOCAL_PROFILE.md).

## Run

```text
sentinel-agent --database sentinel.db --generation local-1 --channel Security --timeout-ms 1000
```

All flags are optional; the values above are the defaults. The process runs until stopped, reads eligible 4624/4625 records from the channel and commits each normalized record with its bookmark in one SQLite transaction.

The service identity must be able to read the Security channel. The documented target is membership in `Event Log Readers` **without** `SeSecurityPrivilege`, which would also allow clearing the log and changing audit policy. That privilege matrix has not yet been verified on a clean VM. The channel should be raised to 256 MB (`wevtutil sl Security /ms:268435456`) before collection; the local database is not the binding limit.

## State and recovery

One SQLite file in WAL mode holds facts (12 months), compressed raw XML (30 days), the channel bookmark, source generation, gaps and a local audit table. `raw` entries are deleted by day and expired facts become daily counts; deletion is recorded and followed by `PRAGMA incremental_vacuum`.

- On first start, subscription begins at the current tail and records that coverage boundary.
- On restart, the persisted bookmark resumes after the last admitted record.
- If the bookmark is rejected, the agent starts a new source generation at the current tail and records `stale_bookmark` with an **unknown** loss count. Record numbers can restart after a channel clear, so they are never treated as globally unique.
- Channel health is degraded below 64 MB of configured size or below 24 hours of coverage; the reason is recorded as a gap and cleared on recovery. A smaller channel or shorter horizon can destroy unread evidence during downtime.

## Not implemented yet

Windows service installation and removal, ACL configuration, the loopback query API, the browser explorer, backup/restore tooling, and measured resource budgets. Native collection has only been compiled and unit-tested here; it still requires the clean-VM privilege and rollover tests described in the [testing strategy](TESTING_STRATEGY.md) before release acceptance.
