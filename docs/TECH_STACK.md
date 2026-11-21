# Technology decisions

| Backend option | Endpoint and operations | Analytics and maintenance | Decision |
| --- | --- | --- | --- |
| All Rust | One language for native collectors, ingest and rules. Small deployment surface. Windows FFI and async cancellation require careful ownership. | Statistical baselines are feasible. Maintaining feature parity with research libraries adds work. | Use for the entire MVP runtime apart from the browser. |
| Rust plus Python | Preserve the same agent and ingest path. Run an isolated worker only for deployments enabling ML. | scikit-learn shortens evaluation cycles. A second runtime adds packaging, schema compatibility and model provenance obligations. | Long-term recommendation once a model beats the baseline. |
| Python first | Rapid parser and API iteration. Native service packaging and per-agent memory need early measurement. | Strong research ecosystem. Rewriting an endpoint later creates a second collector lifecycle. | Suitable for experiments. Less aligned with the endpoint and Rust systems goals. |

## Dependency decisions

| Choice | When and why |
| --- | --- |
| Rust, Tokio, Axum, Serde | Shared typed protocol, bounded async I/O and separate HTTP listeners. Keep blocking Event Log work outside the executor. Enforce limits before deserialization. [Tokio](https://tokio.rs/tokio/tutorial) · [Axum](https://docs.rs/axum/latest/axum/) · [Serde](https://serde.rs/) |
| windows-rs and SQLite | Wrap native handles in an owned collector adapter. Use one local database to commit records and bookmarks together. Measure durable-write cost on Windows. [Windows bindings](https://github.com/microsoft/windows-rs) · [SQLite transactions](https://sqlite.org/lang_transaction.html) |
| PostgreSQL and SQLx | One transaction can store events and, later, pending detection work. Explicit SQL keeps constraints and query plans inspectable. [PostgreSQL](https://www.postgresql.org/docs/current/) · [SQLx](https://docs.rs/sqlx/latest/sqlx/) |
| NATS and Redis | Defer both. Evaluate JetStream when independent durable consumers or replay isolation outgrow the database worker. Measure consumer lag first. A cache needs measured read pressure, expiry and invalidation semantics. [JetStream](https://docs.nats.io/concepts/jetstream) · [Redis](https://redis.io/docs/latest/) |
| React and TypeScript | Filters, detail panels and linked timelines warrant a typed UI. Start with plain CSS. Tailwind is an optional authoring choice after a component vocabulary emerges. [React](https://react.dev/) · [TypeScript](https://www.typescriptlang.org/docs/) · [Tailwind](https://tailwindcss.com/docs) |
| Docker Compose | Reproducible single-host server, database and identity provider. Native agents need access to host logs and service permissions. [Compose](https://docs.docker.com/compose/) |
| Prometheus, Grafana, OpenTelemetry | Expose bounded-cardinality counters and histograms in the MVP. Supply optional Prometheus/Grafana profiles for benchmarks. Add distributed tracing when multiple processes make latency attribution difficult. [Prometheus](https://prometheus.io/docs/introduction/overview/) · [Grafana](https://grafana.com/docs/) · [OpenTelemetry](https://opentelemetry.io/docs/) |
| Aya | Linux-only optional process/network enrichment after validating kernel support, privilege, loss counters and overhead. [Aya book](https://aya-rs.dev/book/) |
| scikit-learn and PyTorch | Compare Isolation Forest and novelty-mode LOF against seasonal robust statistics. Defer PyTorch until labelled data and an explicit deep-learning hypothesis justify training and inference costs. [scikit-learn](https://scikit-learn.org/stable/modules/outlier_detection.html) · [PyTorch](https://docs.pytorch.org/docs/stable/index.html) |

Pin exact dependency and OS versions with the first build. Select versions by supported toolchain, security maintenance, license compatibility and passing platform tests.
