# `minicon-harness.com` package and delivery

The `.com` packaging method is owned by the sibling minicon repository
(`loader/` in `partnernetsoftware/minicon`, PRD_02_27 "con delivery"). This
module records what minicon-harness owns: its cells, its ceiling, its courts
and its Release shape. Method facts are pointed at, not restated.

## Package identity

- [v] the worker is an independently owned crate with its own dependency
  graph (`serde_json`, `ureq`; nothing else), not a bin of another workspace.
- [-] six native cells, one per OS/ISA, all `--profile release` (thin LTO,
  strip, one codegen unit). Unlike minicon's GUI cells, no cell needs winit,
  WebKitGTK or WebView2: the worker is std-plus-two-crates, so every cell
  cross-compiles with one toolchain lane.
- [v] the APE `minicon-harness.com` is a Cosmopolitan fat trampoline plus a
  ZipOS overlay `/zip/cells/{os}-{isa}/`. It does not recompile the worker
  against cosmopolitan libc. The trampoline picks `{os}-{isa}`, copies the
  blob out of ZipOS (or `MINICON_HARNESS_CELLS`) into a per-invocation
  private `mkdtemp` directory, `exec`s it, and cleans up after a successful
  wait.
- [_] Authenticode signability: the APE carries link-time padding for the
  fourteenth PE data directory and a VERSIONINFO resource, so a later signed
  Candidate does not move headers. 0.0.1 publishes unsigned bytes.
- [no] auto-raising the size ceiling. Raising it is an explicit, dated owner
  decision recorded here.

## Artifact budget

- [v] `minicon-harness.com` 0.0.1 Candidate ceiling: initially stamped
  5,242,880 bytes (5 MiB) on 2026-10-27, then **raised the same day to
  6,291,456 bytes (6 MiB) with measured evidence**: the first real pack came
  to 5,619,675 bytes — 376,795 over the 5 MiB stamp — after the size work
  (opt-level=z, fat LTO, panic=abort, ureq tls-only) had already cut the
  cells ~39% (win-x86_64 2,383,360 → 1,454,592 bytes). The remaining bytes
  are ring/rustls TLS repeated across six cells; cutting deeper risks the
  native product. Over ceiling after pack: fail and re-review. Raising it
  again is an explicit, dated, recorded owner decision, never an automatic
  bump.

## Courts (what must pass before a Release)

- [v] six native execute-only courts, one runner per cell: run the packed
  APE bytes themselves (never a rebuilt cell), verifying
  `--version` prints `minicon-harness 0.0.1`, an unknown flag exits nonzero,
  a `file` path outside `--root` is refused, and an `exec` call with an
  empty allow-list is refused without hiding the tool. No cargo, no
  cosmocc, no checkout on the matrix.
- [v] sha256 identity: every court hashes the same downloaded
  `minicon-harness.com` and compares against the pack job's published
  digest and receipt `source_sha`.
- [-] aggregate: all six cells green on one `run_id`, `source_dirty=false`,
  before the release job may publish.

## Release shape

- GitHub Release `v0.0.1` with assets `minicon-harness.com`,
  `minicon-harness.com.sha256`, `build-receipt.json` and `README.md`.
- The Release body is a template rendered verbatim; per-version facts live
  in `prd/archive/`, not in the template.
- Publishing is one workflow that re-verifies the pack receipt, creates the
  tag only if absent, uploads the exact packed bytes, then executes the
  published asset once more from the release download before flipping the
  draft public.
