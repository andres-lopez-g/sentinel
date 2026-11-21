# Project vision

A small security team should be able to reconstruct activity on an enrolled machine, see gaps in its evidence, and explain why a detection fired. The first operator runs one organization on one host. The first analyst searches a fleet of Windows endpoints. Linux becomes the second endpoint platform, with the same enrollment, evidence and query contracts.

## Release boundaries

The Windows pilot starts with Windows 11 x64, a native background service, the Security channel and logon auditing. The central server targets Linux containers, including a Linux VM used by a Windows-based developer. Windows Server and domain-controller coverage need their own audit-policy and volume tests. A Windows host collecting from another Windows host is outside this pilot.

The next platform gate adds one systemd-based Linux distribution and SSH authentication. Later releases add evidence-linked incidents, process and network metadata and opt-in anomaly scoring. A single organization owns its deployment and its keys. Hosted tenancy, high availability and automatic endpoint response would each require a separate design review.

## Portfolio and releases

Publish a tagged walkthrough when a clean machine can complete enrollment, disconnect, reconnect and find the same event by ID. Include a topology diagram, redacted event explorer screenshot, a short replay video, installation and removal steps, Compose prerequisites and the exact tested OS builds. Ship a versioned API contract and upgrade notes with each release.

The strongest recruiting evidence will be a crash-recovery test with a reconciled event manifest, a Windows privilege matrix verified under the actual service identity, and reproducible throughput and memory traces tied to a commit. For ML, publish the comparison against a simple baseline at the same analyst alert budget. Keep sample data synthetic. Use MIT for a low-friction portfolio repository and preserve dependency notices in distributed builds.
