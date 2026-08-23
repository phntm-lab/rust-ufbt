# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — unreleased

Initial release: a native Rust port of
[uFBT](https://github.com/flipperdevices/flipperzero-ufbt), deploying a Flipper Zero SDK
and an ARM GCC toolchain into a local uFBT home directory and building `.fap` / `.fal`
applications, without requiring Python, SCons or a shell.

### Added

- `log` — structured logging: levels, progress reporting with rate limiting, log events
  and pluggable sinks, so a caller can render progress rather than parse text.
- `paths` — layout of the uFBT home directory, including resolution from the environment
  and from a project's `.ufbt` settings.
- `state` — the persisted deployment state written next to a deployed SDK, reproducing the
  `ufbt_state.json` format byte for byte.
- `net` — HTTP downloads over `rustls`, with progress reporting and resumable-looking
  `.part` files.
- `sdk` — the update directory index, deploy tasks, loaders for update channels, firmware
  branches, explicit addresses and local bundles, and the deployer that unpacks an SDK into
  the uFBT home directory.

## Differences from the reference implementation

Behaviour is otherwise identical to uFBT. The following departures are deliberate; each is
a case where the original aborts with an unhandled runtime exception that a caller cannot
meaningfully catch, or where a value has no faithful Rust counterpart.

### The package version is the version of this crate

The reference implementation reports a hard-coded package version that has drifted from the
version its own manifest declares. This crate reports its real version, taken from
`CARGO_PKG_VERSION`. The value is observable through the installer status.

### A deployed SDK without a readable state is replaced, not fatal

When the SDK directory exists but the deployment state next to it is missing or unreadable,
the reference implementation aborts with a path-not-found error, and the half-written
deployment cannot be repaired without removing the directory by hand.

This crate treats such a deployment as an SDK whose version cannot be determined and
replaces it, which is what it already does for a version recorded as `unknown` or `local`.

### State values that are not strings read as absent

The well-known getters of the deployment state — the hardware target, the mode, the
version, the index address and the channel — read a value that is not a string as absent
rather than raising a type error. As above, an unreadable version leads to the SDK being
replaced instead of the deployment failing.

### Malformed directory index fields read as absent

Parsing the `directory.json` index published by the update server never fails on an
unexpected shape. A field of the wrong type, an array that is not an array, and a document
that is not an object all read as absent, and an array element that is not an object is
skipped. The reference implementation raises an uncaught type error for each of these,
which fails the deployment.

A malformed index therefore surfaces as a meaningful error — most often that the requested
channel is not published — instead of a type error.

### A deploy task missing a parameter is an error, not a panic

A deploy task that names a loader mode without carrying the parameter that mode needs — a
`branch` task without a branch, say, which a truncated state file can produce — is reported
as a missing-parameter error. The reference implementation dereferences the absent value and
fails with an unhandled null-check error at the same point.

For the same reason, reading the previous deploy task from a state file that exists but does
not hold a JSON object yields no previous task, rather than failing.
