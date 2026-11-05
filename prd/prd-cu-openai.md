# `prd-cu-openai.md` — external reference: OpenAI / Codex / ChatGPT computer use

**EXTERNAL REFERENCE — NOT a minicon-harness contract.** Nothing in this file
is a requirement, a commitment or a `[v]` claim for this repository. It is a
structured reading of a third party's computer-use tool surface, written so
`PRD.md` can host an honest capability tree and so 0.0.6 / 0.0.7 scope
decisions have a factual baseline. The derived comparison lives in
`PRD.md` → "CU capability tree"; the version decisions live in
`plan/plan-0.0.6-interaction-closure.md`.

Status: compiled 2026-11-05. Raw corpus (13 SDK type files, the Codex
prompting notebook, official-page excerpts, tier-C notes and an evidence
index) is kept outside the repo in the owner's temporary directory
`C:/Users/wjc2022/AppData/Local/Temp/mh-cu-openai/` (see its `INDEX.md`).

Evidence tiers: **A** = first-party code or page fetched directly; **B** =
first-party page that returned 403/Cloudflare here, recovered from a search
index and cross-checked against A; **C** = third-party synthesis, used for
product-line context only. `platform.openai.com` and `developers.openai.com`
were blocked in this session; the openai-python SDK types (generated from
OpenAI's OpenAPI spec) and the Microsoft Learn integration guide are the
authoritative substitutes and are quoted in the corpus.

## 1. The three names are not three competing tools

| Surface | Layer | Computer-use capability | State (2026-11) |
|---|---|---|---|
| **Codex CLI** | coding agent CLI/TUI | **no native desktop CU**. Tools: `shell_command`, `apply_patch`, `update_plan`, `view_image`, `read_file`, `git`. `view_image` attaches a local image path; it is not a screen capture (A) | active |
| **Codex desktop app** | product | background computer use: "seeing, clicking, and typing with its own cursor", multiple agents in parallel, macOS first (A) | active |
| **ChatGPT agent** | consumer product mode | visual browser + text browser + terminal + image gen + connectors in one VM | **retired** (A: help centre notice) |
| **ChatGPT Work / cloud browser** | successor product | remote browser with its own cookies/session, pauses for login/confirmation | active |
| **Operator** | 2025 product | GUI clicks/forms | folded into agent mode; standalone site gone |
| **Responses API `computer`** | **wire-level tool declaration** | model proposes actions, **your code executes them** | GA; `computer_use_preview` deprecated (A + B) |

The only artifact comparable to our `screen` tool is the last row. Codex and
ChatGPT are *harnesses* built on top of such a tool. Comparing them as three
feature lists would produce a wrong tree, so §2 treats the API tool as the
reference PRD and §§3–4 record the product-layer patterns separately.

## 2. Reference PRD: Responses API `computer` tool

### 2.1 Purpose and responsibility boundary

A specialized model ("computer use") interprets screenshots and proposes UI
actions; the model never performs them. Microsoft Learn (A): "Ultimately the
model is never performing actions itself, it passes representations of
commands and you have to provide the final integration layer that can take
those commands and execute them in your chosen environment." The developer
owns execution, screenshots, confirmation, audit logs and isolation.

### 2.2 Tool declaration

| | Deprecated preview (A: SDK types) | GA (A: SDK types, Microsoft Learn) |
|---|---|---|
| model | `computer-use-preview` | `gpt-5.4` |
| tool | `{"type":"computer_use_preview"}` | `{"type":"computer"}` |
| geometry | `display_width: int`, `display_height: int` (required) | **not in the tool object**; the executor owns geometry |
| environment | `environment: "windows" \| "mac" \| "linux" \| "ubuntu" \| "browser"` (required) | not in the GA tool object |
| truncation | `truncation: "auto"` required (B) | not necessary (B) |
| actions per call | one (B) | batched `actions[]` array (A + B) |

Recommended display: 1440x900 or 1600x900 "for optimal click accuracy" (A).

### 2.3 Action vocabulary (9 types, exact fields — A: `computer_action.py`)

`ComputerAction` is a discriminated union on `type`.

| `type` | Required | Optional | Notes |
|---|---|---|---|
| `click` | `button: "left"\|"right"\|"wheel"\|"back"\|"forward"`, `x: int`, `y: int` | `keys: List[str]` | `wheel`/`back`/`forward` are browser-navigation semantics folded into a button enum |
| `double_click` | `x: int`, `y: int` | `keys` | separate action, not a `click` flag |
| `drag` | `path: List[{x:int, y:int}]` | `keys` | **multi-point path**, not a single from→to pair |
| `keypress` | `keys: List[str]` | — | array = a key combination |
| `move` | `x: int`, `y: int` | `keys` | pointer move |
| `screenshot` | — | — | request a fresh observation |
| `scroll` | `scroll_x: int`, `scroll_y: int`, `x: int`, `y: int` | `keys` | **two axes** |
| `type` | `text: str` | — | text entry |
| `wait` | — | — | no duration field in the SDK; the sample integration supplies `ms` |

`keys` (held modifiers) is available on click, double_click, drag, move and
scroll. There is **no** typed verdict, no readback, no window/control target
and no capability manifest in any action.

### 2.4 Message shapes

`computer_call` (A, Microsoft Learn example; B for the batched form):

```json
{
  "type": "computer_call",
  "id": "cu_...",
  "call_id": "call_...",
  "status": "completed",
  "actions": [ { "type": "click", "button": "left", "x": 135, "y": 193 } ],
  "pending_safety_checks": [
    { "id": "cu_sc_...", "code": "malicious_instructions", "message": "..." }
  ]
}
```

`computer_call_output` (A):

```json
{
  "type": "computer_call_output",
  "call_id": "call_...",
  "output": {
    "type": "computer_screenshot",
    "image_url": "data:image/png;base64,...",
    "detail": "original"
  },
  "acknowledged_safety_checks": [
    { "id": "cu_sc_...", "code": "malicious_instructions", "message": "..." }
  ]
}
```

SDK also allows `output.file_id` instead of `image_url` (A). The initial
request may include `input_text` + `input_image` (base64 PNG, `detail:
"original"`) (A). Output status enum: `completed | incomplete | failed |
in_progress` (A). A third-party note (C) warns that `status: "completed"`
means the model finished generating the call, not that anything was executed.

### 2.5 The loop

1. Send a request declaring the tool; optionally include an initial screenshot.
2. Receive `computer_call` with an `actions[]` array (GA) — "a batched array",
   execute each "in order" (A).
3. Execute the actions in your environment (browser/VM/desktop).
4. Capture a fresh screenshot of the updated state.
5. Return it as `computer_call_output` and continue with
   `previous_response_id` until the model stops or you stop (A).

`previous_response_id` links conversation state but **"does not restore a
browser session, login state, or runtime variables"** (B, repeated in C) —
environment state is entirely the integration's responsibility. Sample code
bounds the loop with `ITERATIONS = 5` and hands control back to a human (A).

### 2.6 Safety checks and acknowledgement (A + B)

| code | trigger | required handling |
|---|---|---|
| `malicious_instructions` | screenshot contains adversarial content that could change model behaviour | pass back as `acknowledged_safety_checks`; end user reviews the action |
| `irrelevant_domain` | current domain is not relevant to the conversation | same; end-user review |
| `sensitive_domain` | sensitive site (banking, etc.) | active monitoring / "watch mode" |

"In all cases where `pending_safety_checks` are returned, actions should be
handed over to the end user to confirm proper model behavior and accuracy"
(A). The application, not the model, collects that confirmation.

### 2.7 "Run safely" policy (B, cross-checked in C)

Four rules: restrict the environment (browser/container isolation, domain and
action allow-lists); treat screen content as untrusted; confirm consequential
actions; bound and verify the run with step/time/cost limits.

"Treat only direct user instructions as permission": third-party content
(web pages, PDFs, emails, calendar invites, chats, tool outputs, on-screen
instructions) is untrusted by default. "Typing sensitive data into a form
counts as transmission." "Your integration must communicate what did and did
not run before asking the model to continue."

Three confirmation levels:

| Level | Examples |
|---|---|
| **Hand-off required** | final step of changing a password; bypassing HTTPS warnings or paywalls |
| **Always confirm at action time** | deleting data; changing permissions/sharing/API keys; solving CAPTCHAs; installing or running newly downloaded software; sending/posting on the user's behalf; subscribing/unsubscribing; financial transactions; changing VPN/OS security/computer password; medical-care actions |
| **Pre-approval in the first prompt can be enough** | logging into a user-named site; accepting browser permission prompts; age verification; third-party "are you sure?" warnings; uploading/moving/renaming files; entering model-generated code; transmitting explicitly approved sensitive data |

Isolation recipe (B + C): `debian:bookworm-slim` with `xfce4`, `xvfb`,
`xdotool`, `imagemagick`, `firefox-esr`, non-root user, `Xvfb :99 -nolisten
tcp`, and `--network none --cap-drop ALL --no-new-privileges --pids-limit
256 --memory 2g --cpus 2 --shm-size 512m`.

### 2.8 Documented limitations and risks (A)

- Experimental / demonstration code; run only in a test environment, on a
  low-privilege VM, with no sensitive data and a human confirming decisions.
- Screenshot scope is whatever the integration captures; the Playwright
  sample excludes the URL bar and browser chrome.
- Screenshot failure is masked by a cached last-successful image — a
  correctness hazard for an observation-driven loop.
- Coordinate validation in the sample clamps out-of-bounds values; key
  mapping is integration-specific and explicitly "isn't an exhaustive list".
- "A final answer does not prove the task succeeded" (C).

## 3. Codex: CLI vs desktop app (A, with C hypotheses clearly marked)

**CLI (A, cookbook notebook):** no desktop-control tool. The closest is
`view_image` ("Attach a local image (by filesystem path) to the conversation
context for this turn") plus `update_plan` and `shell_command`. CU is not part
of the CLI contract.

**Desktop app (A, launch post):** "background computer use ... seeing,
clicking, and typing with its own cursor", "Multiple agents can work on your
Mac in parallel, without interfering with your own work in other apps",
"Computer use is initially available on macOS". Adjacent capabilities: an
in-app browser with on-page annotations, image generation, 90+ plugins
(skills / app integrations / MCP servers), multi-tab terminals, SSH devboxes,
automations and memory (preview).

**Not stated on the fetched launch post** (so recorded as C hypotheses only):
AX-tree perception, Screen Recording + Accessibility grants, per-app
authorisation, forbidden apps, admin `remote_computer_use` switch, GUI actions
escaping the file/command sandbox, regional exclusions. The AX-grant claim is
notable because it corroborates why a GitHub hosted runner cannot honestly
qualify a macOS AX control tree (see the 0.0.6 plan's Decision table) — but it
must not be quoted as an OpenAI-verified fact.

## 4. ChatGPT agent → Work / cloud browser (A + C)

The help centre now says "ChatGPT 智能体已不再提供" and points to ChatGPT Work
and the cloud browser (A). Historical agent mode (A, legacy sections of the
same page):

- Perceived the web through screenshots of a **virtual browser window**;
  clicked, filled forms, navigated, adapted on errors.
- Paused for clarification or confirmation; could be guided or interrupted
  mid-task; high-impact actions required confirmation; prompt-injection
  monitoring; "watch mode" on some sites.
- **Takeover**: on login the user takes over the virtual browser and no
  screenshots are taken during that period; the agent then tries to resume.
- Connectors (Web search, Box, Gmail, Google Calendar), scheduled tasks,
  workspace-level site allow/block lists, compliance logs covering
  conversations but not individual agent actions.
- Explicit warning: these guardrails "并不能消除所有风险"; prompt injection via
  calendar/email content is a documented example.

C-tier context: the 2025 launch combined Operator (visual browser), Deep
Research (text browser), a terminal, image generation and connectors with
shared state in one VM; Work's cloud browser keeps its own cookies/sessions
and continues running when the user closes their device.

## 5. What this contract does **not** specify

This is the part that matters for our design. Compared with `cu-abi 1.0`, the
OpenAI tool surface has no:

- typed verdict (`performed / not_performed / unknown / refused /
  unsupported / stale`) — only a generation `status`;
- pointer or window **readback** after an effect;
- capability manifest / per-OS matrix / named refusal for an unimplemented verb;
- actuation receipt proving a button event was actually dispatched;
- window inventory, display geometry, foreground title or typed control list;
- generation / `as_of` pin / stale-replay protection (it uses
  `previous_response_id`, a different concern);
- compile-time per-OS backend contract, six-cell cross-compile or signed
  release evidence;
- CI-verified real-desktop journey with an independent reader.

Conversely it has things we have deliberately excluded: a model-visible
**image arm** (base64 PNG every turn), batched `actions[]`, explicit
`wait`, two-axis scroll, multi-point drag paths, held-key modifiers, a
browser-navigation button enum, a three-level human-confirmation policy and
`pending_safety_checks`. §6 of `PRD.md` turns both directions into a tree.

## 6. Using this reference

- `PRD.md` → "CU capability tree (benchmarked against this file)" is the
  single place where a CU feature is located, with `[v]/[-]/[_]/[no]` and a
  target version.
- `plan/plan-0.0.6-interaction-closure.md` → "External benchmark" records
  which deltas 0.0.6 absorbs and which it defers.
- No OpenAI field name, action name or policy is copied into our wire schema
  by this document. Any future adoption needs its own PRD and gate.

## 7. Evidence caveats

- Official guide / integration-recipes pages were unreachable directly
  (403 / Cloudflare); their indexed text is tier B and is labelled as such.
- The GA `computer` tool object has only `type` per the SDK, while the
  deprecated preview carried `display_width/height/environment`; do not
  assume the GA tool still declares an environment enum.
- The `wait` action has no duration field in the SDK types; the `ms` field
  exists only in the sample integration code.
- ChatGPT agent's retirement date is not published; a third party verified
  the notice on 2026-10-26, which is not an OpenAI fact.
- No live API call, no sample-app source and no quality benchmark was used.
