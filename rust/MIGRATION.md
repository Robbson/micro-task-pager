# Rust Migration – Progress and Phase Log

This is the living execution document for the Rust migration. `ARCHITECTURE.md` is the contract and
the source of truth for design; this file tracks progress, verification evidence, and decisions that
are not already architectural.

Keep this document short. Update it when a phase starts or completes, when a Windows build is verified,
or when an open question is resolved. Do not duplicate architecture content; link to the relevant
section instead.

## Phase status

| Phase | Status | Started | Completed | Notes |
|---|---|---|---|---|
| 1. Scaffold | In progress | 2026-10-30 | | Crate, toolchain, settings, single instance done; manifest and logging pending |
| 2. Virtual desktop subsystem | Not started | | | Probe and COM tests come first |
| 3. Window model | Not started | | | |
| 4. Main window and rendering | Not started | | | |
| 5. Always-on behavior | Not started | | | |
| 6. Parity and deployment | Not started | | | |

## Phase 1: Scaffold

- [x] Cargo crate in `rust/` with pinned toolchain and `rustfmt`/`clippy`.
- [x] `rust/.gitignore` for `target/`.
- [x] Single-instance mutex with the release mutex name.
- [x] COM STA initialization guard.
- [x] Settings model, defaults, and tolerant registry import.
- [ ] Application manifest, icon, and version resource.
- [ ] Logging to `%LOCALAPPDATA%\MicroTaskPager\logs` with 14-day retention.
- [ ] Reshape the flat `RendererViewModes` registry value into the per-desktop/screen model.
- [ ] Replace the temporary console entry point with the Win32 message loop (Phase 4).
- [x] `cargo fmt`, `cargo clippy -D warnings`, `cargo build`, and `cargo test` pass.

## Decision log

| Date | Decision | Rationale | Reference |
|---|---|---|---|
| 2026-10-30 | GDI renderer first, Direct2D deferred | Layered-window alpha and crate maturity outweigh GPU benefits | ARCHITECTURE §7.1, §7.3 |
| 2026-10-30 | No settings UI in v1 | Product decision; keeps the port small and avoids a GUI framework | ARCHITECTURE §8.1 |
| 2026-10-30 | One safe virtual-desktop API plus per-build vtable modules | The C# variants duplicate logic; only the layouts differ | ARCHITECTURE §6.1 |
| 2026-10-30 | Use `windows` 0.62 and the 0.100 native-app crates where applicable | The umbrella `windows` crate has not moved to 0.100 | ARCHITECTURE §5.1 |
| 2026-10-30 | Use the `stable` toolchain channel for now | The installed `1.99.1` rustup toolchain is missing `cargo.exe`; stable is also 1.99.1 | Open question below |

## Verified Windows builds

Record every build on which the probe runs. Do not infer results from a different edition.

| Build | Edition | Selected variant | Probe result | Date | Notes |
|---|---|---|---|---|---|
| | | | | | |

## Open questions

| Question | Why it matters | Owner | Status |
|---|---|---|---|
| Is the `Win11_` boundary build 22621 or 24H2 (26100)? | Wrong selection breaks all COM calls | | Open |
| Which manifest/icon embedding approach: `embed-resource` or linker args from `build.rs`? | Affects build dependencies and reproducibility | | Open |
| Which logging stack: `tracing` + appender, or a small in-house file logger? | Retention and async behavior | | Open |
| Does `IVirtualDesktopPinnedApps` pinning still require `ShowInTaskbar` on current builds? | Always-on behavior depends on it | | Open |
| Should the toolchain be pinned to an exact version? | `1.99.1` is currently installed without `cargo.exe`, so `stable` (also 1.99.1) is used | | Open |

## Verification log

Add short entries, one per meaningful session or phase boundary. Include the command result, not a
narrative.

| Date | Phase | Evidence |
|---|---|---|
| 2026-10-30 | 1 | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`, `cargo test` pass on stable 1.99.1; 4 unit tests; `cargo run` reads the existing `HKCU\SOFTWARE\MicroTaskPager` values and prints them |

## How to use this document

- Check off tasks only when the change is merged and verified, not when it is planned.
- Add new rows to the decision log only for decisions that constrain future phases; routine choices
  belong in the commit message.
- Add a row to the Windows matrix only after running the probe on that build; include the date.
- When a phase completes, summarize the outcome in one or two sentences in the phase status table.
- When a deviation from the C# behavior is implemented, add it to `ARCHITECTURE.md` §10 and reference
  it here.
- Do not create one progress file per phase. Phase documents go stale and duplicate the checklist;
  use commits and the checklists in this file instead.
