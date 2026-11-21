# Requirements and MVP review

The proposed pilot uses one central host, one organization, Windows 11 x64 endpoints and two operator roles (reader and administrator). An administrator configures successful and failed logon auditing in the lab. The collector reports its effective access and source health. Empty results alone cannot establish that auditing is enabled.

## MVP acceptance matrix

| ID | User-visible requirement | Required evidence for approval of the release |
| --- | --- | --- |
| W1 | Install a Windows service with scoped read access. | On a clean Windows VM, start, stop, reboot and remove it. Capture the account, channel ACL and audit policy. A denied read produces degraded health. |
| W2 | Collect Security events 4624 and 4625. | A controlled successful and failed logon appear with the expected outcome, target identity, endpoint and source record. Parse named XML fields across fixture versions and locales. Missing optional IP remains absent. |
| W3 | Protect identity in transit. | Two agents have distinct credentials. An expired or revoked enrollment fails. An active agent cannot submit as the other. Reader cannot enroll devices. |
| W4 | Survive service and network interruption. | Reconcile all eligible fixture IDs across an agent restart, a ten-minute outage and a lost acknowledgement. Exactly one stored row per admitted ID within retention. |
| W5 | Report data gaps. | Force a stale bookmark, access denial and full spool independently. Record source, reason, last good position and recovery time. Show unknown loss counts as unknown. |
| W6 | Support daily investigation. | Filter by time, endpoint, event code and outcome, page through consistent results, open evidence detail and distinguish source time from receipt time. Show endpoints, last heartbeat, ingest rate and collector degradation. |
| W7 | Bound resource use and retention. | Pass the steady-load, endpoint, disk and recovery gates in the testing strategy. Expired test rows disappear and deletion is audited. |
| W8 | Reproduce the pilot. | A second operator follows the pinned Compose and Windows service guides using synthetic identities. Restore a backup in an isolated environment and find a known retained event. |

## Later capabilities

Phase 2 adds Linux inventory and SSH/system journal collection with cursor recovery. Phase 3 adds versioned rules, correlation, analyst notes, assignment, status, risk, evidence and investigation history. Its dashboard adds alert distribution, severity, detection history, incident timelines and per-endpoint activity. Advanced monitoring adds process start/exit, network metadata and sensitive file changes where configured. Phase 4 evaluates statistical and ML signals. Phase 5 adds optional cited explanations.

**Approval record:** pending maintainer review of W1–W8, the single-host pilot, retention budget and source coverage. Approving this plan authorizes implementation planning gates to advance. Release acceptance requires the test evidence above.
