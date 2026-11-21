# HTTP contract proposal · v1

Separate agent and analyst listeners. JSON requests use UTF-8. Unknown major versions fail. Event schema version evolves independently of the URL. Bind sessions to roles server-side. Return a server request ID on all HTTP responses. Errors use a stable code, message and optional item indexes. Logs contain the request ID.

| Method and route | Caller | Request and successful result |
| --- | --- | --- |
| POST /v1/enrollments | Administrator | Platform, display name → 201 with endpoint UUID and single-use token expiring in 10 min. |
| POST /v1/agents/enroll | Bootstrap token over server-authenticated TLS | CSR and token → 201 with certificate and expiry. On lost response an administrator can invalidate and reissue enrollment. |
| POST /v1/agents/renew | Active mTLS agent | Fresh CSR → 201 for same endpoint, bounded overlap for rotation. |
| POST /v1/events/batches | Active mTLS agent | Object with events array using the data model → 200 receipt after commit. |
| POST /v1/heartbeat | Active mTLS agent | Agent version, source state, gap reason and spool usage → 204. Server sets last-seen time. |
| GET /v1/endpoints, GET /v1/events | Reader or higher | Paginated items and next_cursor. Event filters: endpoint_id, from, to, code, outcome. |
| GET /v1/endpoints/{endpoint}/events/{id} | Reader or higher | 200 normalized evidence and provenance, 404 if absent or expired. |
| POST /v1/endpoints/{endpoint}/disable | Administrator | Reason → 204. Audit the change and reject subsequent agent requests. |
| GET /v1/incidents/{id} | Reader or higher, Phase 3 | Version, timeline, evidence IDs, score breakdown and analyst disposition. |
| PATCH /v1/incidents/{id}, POST /v1/incidents/{id}/notes | Analyst or administrator, Phase 3 | Require expected revision on edits and idempotency key on notes. 409 on stale revision. Append history atomically. |

## Batch semantics

Start with 1–200 events, 512 KiB uncompressed UTF-8 per batch, 16 KiB per item and no compression in v1. Cap nested objects at eight levels, strings at 2 KiB. Rate-limit by credential and reserve global capacity. Reject new events older than seven days by occurrence time. Future times beyond five minutes receive a clock-quality flag. Deduplication applies for the receipt-based retention period. Lookup a retained ID before testing age so retry of an accepted item can succeed.

Validate the entire batch before committing. A successful response such as `{"inserted":18,"duplicates":2}` accounts for every item. A duplicate must match the normalized client payload digest. On 422 (invalid items) or 409 (ID conflicts), roll back the whole batch and identify offending indexes. Agent quarantines those records and resubmits the remainder. Compare-and-insert under the unique key inside the transaction to handle concurrent retries. On 413 split the batch, quarantine an oversize singleton. On 429 or 503 honor Retry-After plus jitter. Retry unknown commit outcomes using the original IDs. On authentication failure suspend sending, retain the spool and surface credential health.

Page size defaults to 50, maximum 200. Order events by descending receipt time, endpoint UUID and event UUID. Return an opaque authenticated cursor binding filters, sort tuple and first-page upper bound. This bounds a live view across pages. Retention may remove older rows during paging. Use a fresh query for live updates. Require bounded time ranges and reject invalid filters with 400. Deny missing credentials with 401 and unauthorized roles with 403 where HTTP authentication applies. TLS handshakes can fail before an HTTP response exists.

Incident transitions: open → investigating → resolved → closed. Reopening requires an analyst reason. Record actor, UTC time and old/new state, synchronize severity with linked alerts and preserve overrides. Publish a machine-readable OpenAPI contract and test examples alongside Phase 1 routes.
