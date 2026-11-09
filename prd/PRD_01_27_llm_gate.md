# PRD 01.27 — LLM gate contract (M-line, 0.0.20)

Status: `[_]` contract for a version whose code does not exist. This file owns
the **contract** for 0.0.20; `plan/v0.0.20.md` owns the work breakdown and
`PRD.md` owns the status. It changes no source, no version, no `[v]`, no `[no]`
row and no published asset. Legend: `[v]` delivered with evidence, `[_]`
planned, `[no]` non-goal.

## Domain fit

`minicon-gate` is the family's **mouth**: it holds the tuple that decides where
a model call goes and out through which pipe, so no client has to. It sits
outside the worker core, beside the MCP adapter, and inherits the core rules it
does not restate: BYOK (the operator owns every credential), a bounded surface,
and typed evidence a reader recomputes.

It is **not** part of the KVM + W + S domain of `PRD_01_08`. It does not use
`screen`, does not post input, reads no process or memory, and touches no
device. It owns no host authority. Its only authority is over **one outbound
HTTP exchange it was asked to perform**.

## Why the gate exists

The measured failure: OpenCode desktop reads one process-wide `HTTP_PROXY`.
That is all-or-nothing across providers, cannot be scoped to one account, and
breaks when the proxy is on loopback. No client-side fix exists, because the
decision lives in an environment variable the gate replaces with **data**.

The gate is therefore legitimate only while it holds a decision a client cannot
make. Concretely, it must remain better than a client-local config in at least
one of: per-lane egress, multiple accounts per provider, schema-validated
workflow bindings. If it stops doing any of those it is a worse LiteLLM.

## Three planes, one process

```text
client --(loopback, never proxied)--> gate --(lane egress)--> upstream
```

| Plane | Contents | Rule |
|---|---|---|
| definition | endpoints, credentials, egress, lanes, agents, workflows | data only, revisioned |
| resolution | intent -> ordered lane candidates | **pure function, no I/O** |
| transport | one attempt: secret, tunnel, dispatch, classify | bounded, one lane at a time |

A workflow is **declared here and executed elsewhere**. The gate validates it,
resolves every node, serves the plan, and never advances a node.

## The pool is three axes, not one flat lane

A flat lane `(account, model, modelArgs, proxy)` is rejected as the model. It
forces `N endpoints x M credentials x K egress` copies of the same bytes and
makes account rotation a rewrite of every lane. The pool is:

```text
endpoint    { provider, baseURL, protocol, headers? }
credential  { account, label, secretRef, scope }
egress      { proxy, noProxy, timeouts }
lane        endpoint x credential x egress + model + modelArgs + limits
```

Axis-level expansion is the supported way to write many lanes:

```jsonc
"lanes": {
  "oa-sol": {
    "endpoint": "oa", "model": "gpt-6.1-sol",
    "credentialOrder": ["oa-personal", "oa-work"],
    "egressOrder":     ["proxy8891", "direct"]
  }
}
```

Expansion is the **Kronecker product**: credential outer, egress inner, both in
declared order, then deduplicated by `(endpoint, credential, egress)`. The
result is deterministic, so an independent reader recomputes it from the
revision alone. A named lane is the same product of one.

Why three axes and not one: a flat lane can only cool down a whole lane. A
dead proxy cools every lane through it and leaves `direct` untouched; a dead
credential cools only itself. This is an evidence property before it is an
efficiency property — the reader must be able to say **which axis failed**.

## Precedence

```text
gate defaults < credential.scope < lane < agent < workflow node < request
```

- Objects (`modelArgs`, `timeout`) merge deep; later scalars win.
- `headers` match case-insensitively; later values replace.
- Arrays replace wholesale, never concatenate.
- `proxy`, `credential`, `endpoint`, `baseURL`, `protocol` and `model` are
  **operator-only**: a request may change model arguments, never the pipe.
- A protected override is `refused` naming the field, not ignored.
- `workflow node > agent` is the one precedence line a workflow may use; it
  may set `modelArgs` and `egress` only.

## Wire surface

Ingress is OpenAI-compatible over loopback:

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/v1/chat/completions` | one model call, non-stream or SSE pass-through |
| `GET` | `/v1/models` | virtual models `gate/<agent-id>`, `gate/lane/<lane-id>` |
| `GET` | `/gate/v1/health` | liveness and config revision |
| `GET` | `/gate/v1/pool` | endpoints, credentials, egress, lanes, redacted |
| `GET` | `/gate/v1/agents` | agent bindings and resolved order |
| `GET` | `/gate/v1/workflows` | workflow DAGs |
| `POST` | `/gate/v1/resolve` | `{agent|workflow, overrides}` -> ordered plan, no dispatch |
| `GET` | `/gate/v1/events` | bounded evidence rows |

- `/gate/v1/*` is read-only by contract. Editing the config file is the only
  management verb, and `agenterm` owns a UI if one is ever wanted.
- `stream: true` is a **pass-through**: bytes relay as they arrive, hop-by-hop
  headers are stripped, the final usage chunk is recorded and not rewritten.
- Response headers are evidence, not control: `x-gate-lane`, `x-gate-attempt`,
  `x-gate-config-revision`, `x-gate-egress` (host only).
- The client `Authorization` authenticates to the gate and is **never
  forwarded**. Upstream auth comes from the lane's credential, or, for
  `credential: "client"`, from a dedicated `x-gate-upstream-authorization`
  header the gate forwards and never stores.

## Egress

`direct` disables proxy use for that egress even when the gate's own process has
`HTTP(S)_PROXY` set. `env` inherits the process variables, one named egress at
a time and never as a default. An explicit `{url, noProxy}` is the only
multi-lane form.

- HTTPS goes through `CONNECT`, and the **tunnel is established by the gate**,
  not by the HTTP client. A per-lane proxy cannot be expressed through a
  process-wide or agent-wide HTTP client: the tunnel is the unit of the
  decision.
- `noProxy` is mandatory for an explicit egress and defaults to
  `localhost,127.0.0.1,::1`. An empty list is a validation failure, because the
  gate's own client hop must never be proxied.
- An egress whose `host:port` equals the gate's own listener is a **load-time
  refusal**: a self-loop is a bug, not a configuration.
- A 407 from the proxy is `unknown` and is not retried on another lane with the
  same egress: the pipe is broken, not the lane.
- No redirects are followed. A redirected POST replays a bearer token to a host
  the operator never named.

## Fallback

One request is one **attempt chain**. The chain stops at the first `performed`.

| Outcome | Verdict | Fallback | Axis cooled |
|---|---|---|---|
| upstream 2xx | performed | stop | none |
| upstream 429 | refused | yes | credential, 60 s |
| upstream 5xx | refused | yes | egress, 30 s |
| upstream 401 / 403 | refused | no | credential, 300 s |
| upstream 400 / 404 / 422 | refused | no | none |
| connect / TLS / tunnel failure | unknown | yes | egress, 60 s |
| proxy 407 | unknown | no | egress, 300 s |
| secret unreadable | unknown | no | credential, until config change |
| non-stream body over bound | unknown | yes | lane, 30 s |
| config revision changed mid-request | stale | no | none |

A non-429 4xx never falls back: the next lane would answer identically, and a
bad request repeated N times is N times the cost for one decision.

Cooling changes **order**, never admission. A cooling lane is moved to the
back of the candidate list, not dropped, so the reader can recompute why it was
deprioritised rather than absent.

## Evidence

One typed row per attempt, in the repo's existing verdict vocabulary:

```jsonc
{
  "ts": "2026-11-09T00:00:00Z",
  "request_id": "...", "attempt": 2,
  "triggered_by": 1, "trigger_reason": "upstream_429",
  "workflow_id": "refactor", "step_id": "apply", "task_id": "...",
  "agent_id": "planner", "virtual_model": "gate/planner",
  "endpoint_id": "oa", "credential_id": "oa-work", "egress_id": "proxy8891",
  "lane_id": "oa-sol", "provider": "openai", "model": "gpt-6.1-sol",
  "egress_host": "127.0.0.1:8891",
  "verdict": "performed", "status": 200, "error_class": null,
  "lane_state_before": "healthy", "lane_state_after": "cooling",
  "cooldown_until": null,
  "latency_ms": 812, "usage": { "input": 1200, "output": 340 },
  "config_revision": "sha256:...", "lane_revision": "sha256:..."
}
```

- `egress_host` is a host and port, never a URL: a proxy URL may carry
  credentials and those bytes appear nowhere else.
- `credential_id` is the pool key, never the secret and never the label alone.
- `triggered_by` makes the chain explicit; a reader never infers causality from
  adjacent rows.

**Reader contract (E2 equivalent).** Given the recorded config revision, the
axis definitions under it, and the attempt rows, an independent reader must
recompute (a) the expanded candidate order for the agent; (b) which attempt was
chosen and why each earlier candidate was skipped or demoted; (c) that the
recorded egress host equals the egress axis's host; (d) that no row contains a
credential-shaped value. The reader is a CI script, not a trust claim.

A **JSONL mirror is required** whenever `events.mirror` names a path: the ring
alone cannot support offline recomputation after the process exits.

## Bounds (transport bounds, not policy)

- Config <= 1 MiB; 256 endpoints, 256 credentials, 64 egress, 512 lanes,
  256 agents, 64 workflows, 64 nodes each.
- Expanded candidates per agent <= 32, after the Kronecker product.
- <= 4 attempts per request; <= 15 minutes total; <= 8 MiB request;
  <= 1 MiB per SSE event; <= 4 MiB non-stream response.
- Per-lane concurrency 1..16, queue 64, then `429` backpressure.
- Events: 10,000-row ring, current revision only; reload is atomic and
  revision-pinned, and an in-flight request is never re-resolved mid-stream.

A cooling lane plus a queue-full lane can leave zero candidates. That is a
local `429` with a named reason, never a silent downgrade to another pipe.

## Admission and honesty

1. A lane whose `protocol` cannot express the request is `unsupported`, not a
   best-effort rewrite. 0.0.20 ships `openai-chat` only.
2. A credential whose `provider` does not match the endpoint's is a load-time
   refusal, not a runtime surprise.
3. A bound is a bound in every build. No test mode relaxes a numeric bound.
4. Nothing here is `[v]`. Only three-OS CI, the independent reader and
   owner-view acceptance produce a claim.
5. A request that cannot be routed safely is `refused` with the reason, and the
   reason never contains a secret, a proxy credential or a URL with userinfo.

## Non-goals

- `[no]` tool execution, a policy engine, a plugin runtime, a management UI, a
  workflow executor or scheduler, a second model loop, request or response
  caching.
- `[no]` budgets, rate limits, key issue/revoke, content guards. Cooling and
  concurrency are transport state: they reorder or refuse one attempt, they do
  not decide whether a task may run.
- `[no]` credentials at rest. `env` and `file` resolve at egress and are held
  for one request. `oauthFile` and `command` are named in the schema and
  refused with a "not in this slice" reason so the schema does not churn.
- `[no]` proxy auto-discovery, PAC files, SOCKS, or a second listener. The gate
  binds one loopback address and never another.
- `[no]` a provider metadata table. `provider` is a string label; `model` is
  declared per lane.
- `[no]` request rewriting beyond protocol conversion. A `stream:true` request
  is relayed, not re-framed; a non-stream request body is forwarded unchanged
  except for the operator-owned fields.
