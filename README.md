# Sentinel

Sentinel is a planned, self-hosted security information and event management platform for Windows and Linux. The first release will help an analyst answer a concrete question: **which Windows logons succeeded or failed, on which endpoint, and was collection healthy at the time?**

**Status · Phase 1 in progress on the [single-host profile](docs/LOCAL_PROFILE.md).** The design remains the reviewed baseline. Two crates work today: `crates/protocol` provides the v1 event model, admission validation and the Windows 4624/4625 XML parser; `crates/agent-core` provides the WAL SQLite spool, atomic bookmarks, source generations, gap accounting and retention. Both are covered by synthetic fixtures and contract tests. Installation instructions, binaries and measured performance will accompany working releases.

| Read first | Decision it supports |
| --- | --- |
| [Project vision](docs/PROJECT_VISION.md) | Audience, release scope and evidence for the portfolio |
| [Local profile](docs/LOCAL_PROFILE.md) | Why the first release has one host and no server |
| [Requirements and MVP acceptance](docs/REQUIREMENTS.md) | What a Windows pilot must demonstrate |
| [Architecture](docs/ARCHITECTURE.md) and [technology choices](docs/TECH_STACK.md) | Trust boundaries, delivery and operating costs |
| [Threat model](docs/THREAT_MODEL.md) | Which claims an analyst can trust |
| [Data model](docs/DATA_MODEL.md) and [API contract](docs/API_SPECIFICATION.md) | Event identity, durable receipts and analyst access |
| [Roadmap](docs/ROADMAP.md) | Phase gates from collection to detection and release |
| [Repository structure](docs/REPOSITORY_STRUCTURE.md) | Component boundaries and engineering standards |
| [Testing strategy](docs/TESTING_STRATEGY.md) | Reproducible evidence and proposed performance budgets |

Review the Windows scope, delivery contract and acceptance matrix together before approving Phase 1. See [contribution guidelines](CONTRIBUTING.md) and the [MIT license](LICENSE).
