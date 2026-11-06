# 0.0.9 — grant and environment disclosure: a read-only, non-elevating report

Status: `[v]` **shipped as a public signed Release**; W1-W4 are closed. Planned after `v0.0.8` was published as a public signed Release and its owner-view acceptance completed; W4 landed in `99ea15729dde762628977077790fbbb1a7e2f44a`.

Baseline: `v0.0.8` widens `act:key` with bounded `hold` / `release`; the
capability matrix is windows 12/12, linux 10/12, macos 8/12, other 0/12,
`cu-abi 1.0` is frozen, and `scripts/cu-cfg-check.py` type-checks four
backend configurations. Four consecutive versions (0.0.5 through 0.0.8)
delivered desktop verbs without declaring a new capability row.

## Decision: what 0.0.9 is, and what it deliberately is not

The owner's direction is unchanged: **concentrate on computer use; do not
grow harness capabilities**. Of the deferred chapters, the only one that is
pure *observation* -- no new native effect, no new grant, no loop/budget
decision, no policy engine -- is the environment and grant half of E3/E4:

| Delivered in 0.0.9 | Leaf | Shape |
|---|---|---|
| grant + environment disclosure | E3 / E4 | `op=disclose` gains structured `environment` and `grants[]`; every probe is read-only, non-elevating and per-backend |

| Deferred, with the reason recorded so it is not re-litigated | Unlock condition |
|---|---|
| explicit bounded `wait` (B7b) | a loop/budget decision (D4), **not** a desktop verb; it would add a 13th capability row and change every reader's `EXPECTED` |
| batched `actions[]` (B8) | one call with several effects conflicts with "one act -> one verdict -> one receipt" and with `as_of` pinning; needs its own admission/stale design |
| a11y trees (0.7.4), image arm (0.7.1), safety policy (0.7.2), app-level effect evidence (0.7.6) | unchanged unlock conditions in `PRD.md`; none is met by a hosted runner today |
| held-key effect evidence (a click *while* a modifier is down) | app-level state change; needs a side-effect-free echo fixture on three OSes (F3) |

Consequence: **the capability matrix still does not grow**, for the fourth
consecutive version. `environment` / `grants` are additive evidence fields on
the existing `disclose` capability, not a new capability name. `cu-abi` stays
1.0. `disclose` remains pure information: a caller may read it even when
computer use was never granted.

## Contract owner: `prd/PRD_01_11_grant_disclosure.md`

- `disclose` adds `evidence["environment"]` (`platform`, `session`,
  `display_count`, `virtual_desktop`, `foreground_present`) and
  `evidence["grants"]` (one row per named probe, with a typed status and an
  optional reason). No window titles, paths or other identifying strings are
  added by this version.
- Status is one of `granted`, `denied`, `unknown`, `not_applicable`.
  `unknown` means "could not be probed safely" and always carries a reason;
  it must never be reported as `granted`, and `denied` must never be
  auto-escalated.
- **No probe may request a permission.** Preflight / read-only APIs only:
  `CGPreflightScreenRecordingAccess`, `CGPreflightPostEventAccess` and
  `AXIsProcessTrusted` (never the `WithOptions` prompt) on macOS; `DISPLAY`,
  `XDG_SESSION_TYPE` and a read-only `XOpenDisplay` / `dlsym` check on Linux;
  process integrity and `GetForegroundWindow` on Windows. The fallback
  reports every probe `not_applicable` with a named reason.
- One function set, four configurations: `native::environment()` and
  `native::grants()` change together on Windows, macOS, Linux and the
  fallback backend; `scripts/cu-cfg-check.py` type-checks all four.
- 0.0.6 / 0.0.7 / 0.0.8 semantics are untouched: every reader assertion
  about receipts, `path_points`, both scroll axes, held keys, generations,
  the live final frame, the declared-but-skipped rule and BOUNDARY stays.

## Work breakdown and gates

```text
0.0.9 — grant and environment disclosure
├── [v] W1 — the read-only probe contract
│   files: src/cu.rs, src/wire.rs, src/screen.rs, src/screen_macos.rs,
│          src/screen_linux.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi 1.0, 12-row matrix unchanged, four cfgs type-check,
│   cargo test --locked green on all three CI jobs, no new capability row
│   evidence: cu-platforms.yml run 42164845780, matrix test still 12/10/8/0
├── [v] W2 — a real disclosure journey in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: required disclose_grants on all three OSes; the reader
│   recomputes every probe's status, uniqueness and reason, and checks the
│   grant rows do not contradict the effects that the same journey performed
│   evidence: cu-journeys.yml run 42164845836, three cu-journey.json artifacts
├── [v] W3 — the truth of the docs
│   files: PRD.md, README.md, prd/PRD_01_03_computer_use.md,
│   prd/PRD_01_08_computer_use_scope.md, prd/PRD_01_11_grant_disclosure.md,
│   release notes
│   invariant: E3/E4 marked delivered, B7b/B8 and the 0.7.x chapters still
│   deferred with unlock conditions, no stale "planned" claim for a shipped arm
└── [v] W4 — 0.0.9 version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the four release invariants must not regress
    evidence: run IDs + owner-view anonymous download and re-hash
```

### W2 step design

| New step | Request | Required on | Why it is harmless |
|---|---|---|---|
| `disclose_grants` | `op=disclose` | all 3 | pure read-only preflight; requests no permission and posts no input |

The independent reader is extended, not weakened:

- `EXPECTED` stays exactly 12 rows with windows 12 / linux 10 / macos 8 true.
- `grants` is a non-empty array; each row has a unique `name`, a `status` in
  the four-value whitelist, and a non-empty `reason` whenever the status is
  `unknown` or `not_applicable`.
- At least two named probes are present on each OS; a row whose name implies
  an effect that the same journey performed (for example Linux `x11_display`
  after a successful `observe`) must not be `denied`.
- `environment` publishes `platform` equal to the journey's own platform,
  a non-empty `session`, a positive `display_count` and a boolean
  `foreground_present`; `virtual_desktop` has positive width and height.
- A `BOUNDARY` note must never excuse a missing disclosure.
- Every 0.0.6/0.0.7/0.0.8 assertion stays.

### W4 release invariants (must not regress)

1. `publish` keeps `environment: release` with `github.token`.
2. Signed Windows cells are rewritten into both Windows zips and their
   `.sha256` sidecars are recomputed.
3. `post-publish` re-hashes the **downloaded** assets against
   `signing-receipt.json` `after_sha256`.
4. Defender uses `MpCmdRun -File` and `TAG` resolves through
   `needs: [verify, ...]`.

Chain, all `workflow_dispatch` on `main`, one shared `head_sha`:

```
minicon-harness-com.yml pack -> candidate.yml (source_sha, pack_run_id)
-> company-signing.yml (qualification_only=false) -> release.yml dry_run=true
-> release.yml dry_run=false (publish + Defender + post-publish)
```

Owner-view acceptance is the last step, not the exit code: anonymous HTTP 200
on the Release page and one asset, a fresh download of all 15 assets re-hashed
locally against the receipt and sidecars, and the downloaded APE printing
`minicon-harness 0.0.9`.

## Risks and downgrade rules

- **A permission prompt is the primary risk.** Only preflight / read-only
  symbols may be resolved. If a symbol is missing, the row is `unknown` with
  the library and symbol named; no `WithOptions` call, no TCC dialog.
- **Privacy**: no window titles, user names, paths or environment values
  beyond the session *kind* are published. A probe reports a status, never a
  secret.
- **Honesty over completeness**: an OS that cannot safely answer a probe
  reports `unknown`, not a plausible `granted`. The reader checks the status
  whitelist and never converts `unknown` into a pass.
- **No assertion is relaxed.** If a probe cannot be qualified on an OS it is
  a named `unknown` with a reason in the journey record; the matrix does not
  change to hide it, and the reader does not drop the check.
- **Scope creep**: no new crate, link-time platform library, tool, ABI bump,
  engine, image arm, policy engine or a11y tree. If W1/W2 needs one, split it
  into a separate PRD rather than widening 0.0.9.

## Non-goals

- `[no]` a11y trees (AX / AT-SPI / UIA COM), image arm in the wire codec,
  Wayland-native protocol, runtime backend selection.
- `[no]` explicit bounded `wait`, batched `actions[]` in this version; each
  has an unlock condition above.
- `[no]` app-level state-change evidence; a self-hosted echo window is a
  separate design and gate.
- `[no]` new crate, runtime, plugin host, bun/WASM/ujs embedding; PRD 31
  readiness stays BLOCKED.
- `[no]` re-publishing or overwriting v0.0.4 / v0.0.5 / v0.0.6 / v0.0.7 /
  v0.0.8 assets.
- `[no]` claiming `[v]` for an arm that only compiled, or relaxing a reader
  assertion to make a journey green.

## Delivery record

| Item | Value |
|---|---|
| W1 + W2 code | commit `f28eae53b1490cc6761f9b6e265c4358e1691278` (`cu: grant/environment disclosure for 0.0.9 (W1/W2)`) |
| Compile gate | run `42164845780` — `success`, all six cells, matrix still windows 12 / linux 10 / macos 8 / other 0 |
| Real-desktop journeys | run `42164845836` — `success`, required `disclose_grants` step on all three OSes |
| Journey artifacts | `cu-journey-linux`, `cu-journey-macos`, `cu-journey-windows` (run `42164845836`), each a `cu-journey.json` the independent reader recomputed |
| W3 docs | swept in the same commit as W4 (this file, `PRD.md`, `README.md`, `PRD_01_03`, `PRD_01_08`, `PRD_01_11`, `.github/release-notes.md`) |
| W4 version bump | four sources moved to `0.0.9` in that one commit |
| Release chain | pack `42166110442` -> candidate `42167224386` -> signing `42167283497` -> dry-run `42167468391` -> publish `42167566196`; every step `success`, one shared `source_sha=99ea157...` |
| Owner-view acceptance | Release page and asset anonymously HTTP 200; 15 assets downloaded; 6 `.sha256` sidecars recomputed OK; `minicon-harness.com` and both signed Windows cells equal `signing-receipt.json` `after_sha256`; downloaded APE prints `minicon-harness 0.0.9` (exit 0) |

W1-W4 are closed on the evidence above, including the controller's own
anonymous download and re-hash. The next version is planned only after this
public signed Release and its owner-view acceptance: see
`plan/plan-0.0.10-display-scale-disclosure.md`.
