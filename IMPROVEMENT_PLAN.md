# Headroom Improvement Plan

## Product rule

Every integration usage check must use an in-process HTTP client against the provider API. Usage collection must never invoke a provider CLI, `curl`, a browser, a PTY, or a local usage database. Local files and Keychain may only be used to discover credentials.

## Baseline audit (before v0.4.0)

### Performance

- Usage refreshes launch `curl` subprocesses and, for OpenCode fallback, `sqlite3`.
- The status-item loop wakes every 50 ms even while the popover is closed.
- Refresh runs every 60 seconds, including a Claude messages probe that should be used sparingly.
- Manual and scheduled refreshes can overlap.
- HTTP connections are not pooled between integrations or refreshes.
- Font discovery runs on every panel render.

### Data integrity

- OpenCode falls back from authoritative HTTP data to a local cost estimate. The UI can therefore present estimates as quota.
- Failed integrations are dropped from the panel, while a partial success advances the global sync time.
- Synthetic burn-rate sparklines look historical but are derived from one current value.
- The UI does not expose whether a value is live, cached, loading, or failed.

### UX/UI

- The first load can show an empty panel and `?` menu-bar state.
- Disconnected integrations disappear rather than offering a clear setup state.
- Refresh has no progress or failure feedback.
- Preferences are not persisted across launches.
- The warning slider is visual only.
- “Add a subscription…” appears interactive but has no action.
- Connected Accounts lists only successful fetches, not configured integrations.
- Secondary limits omit reset details, and long reset labels can crowd rows.

### Reliability and maintainability

- Provider transport, credential access, parsing, and fallback policy share one large file.
- Dead CLI-era parsing and command-resolution code remains.
- The test suite covers payload parsing but not refresh state, error retention, or the HTTP-only invariant.
- README usage-source documentation is stale.

## Delivery plan

### P0 — trustworthy, efficient collection

- [x] Replace `curl` with one pooled, in-process Rust HTTP client.
- [x] Remove the OpenCode SQLite estimate and all non-HTTP usage fallbacks.
- [x] Add bounded connect/request timeouts and response-size limits.
- [x] Prevent overlapping refreshes.
- [x] Refresh on launch/manual action, then at a conservative background interval.
- [x] Replace the permanent 50 ms status poll with event-driven toggles and idle timers.
- [x] Preserve last-good provider data when one integration fails.
- [x] Track per-integration loading, latency, freshness, and errors.

### P1 — honest, finished core UI

- [x] Keep every supported integration visible while loading or disconnected.
- [x] Show API-only provenance, refresh progress, cached/error state, and request latency.
- [x] Remove synthetic history visuals.
- [x] Persist preferences atomically.
- [x] Make the warning threshold interactive.
- [x] Remove non-functional controls.
- [x] Add provider-specific setup actions and credential validation feedback.
- [x] Persist per-integration enable/disable controls and skip disabled API sources entirely.
- [x] Add accessible keyboard focus and labels; the UI has no animated transitions.
- [x] Re-capture release screenshots from live usage and preferences states.

### P2 — deeper performance and resilience

- [x] Record cold start, idle CPU wakeups, resident memory, refresh duration, and binary size in CI artifacts.
- [x] Add per-provider exponential backoff.
- [x] Cache short-lived OAuth access tokens in memory to avoid a token refresh on every cycle.
- [x] Move Keychain access from the `security` command to Security.framework.
- [x] Split provider adapters into separate modules with fixture-based contract tests.
- [x] Add structured, redacted diagnostics and an exportable support report.

### P3 — release quality

- [x] Package an ad-hoc signed `.app` with a stable bundle identifier.
- [ ] Sign and notarize the distribution build with Apple Developer credentials.
- [x] Add update checks and release-channel metadata.
- [ ] Test multi-display placement, menu-bar auto-hide, offline recovery, sleep/wake, and credential rotation.

## Acceptance targets

- Zero child processes spawned by a usage refresh.
- Zero local-database reads used to calculate provider quota.
- No overlapping refresh jobs.
- Closed-popover control loop wakes at most once every five seconds.
- A provider failure never deletes its last-good value.
- Every displayed value visibly distinguishes live, cached, loading, or setup-required state.
- Idle refresh interval is at least five minutes unless the user explicitly refreshes.
- Preferences survive restart; all visible controls perform an action.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, and release build pass.
