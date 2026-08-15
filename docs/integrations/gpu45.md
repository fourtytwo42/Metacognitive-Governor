# GPU45 Development Provider

- Status: Available on the owner's private LAN.
- Last verified: 2026-08-15.
- Purpose: Local model inference and optional media/tool capability testing during
  MCG development.

## Origins

```text
Console:   http://192-168-50-189.nip.io
Inference: http://192-168-50-189.nip.io:30001
Guide:     http://192-168-50-189.nip.io/llms.txt
```

The hostname is non-secret configuration. Credentials and tokens must never be
committed. Store any future management or inference credential through MCG's
credential abstraction and the operating-system credential store.

## MCG provider connection

Configure an OpenAI-compatible provider with base URL:

```text
http://192-168-50-189.nip.io:30001/v1
```

Discovery sequence:

1. `GET http://192-168-50-189.nip.io/api/version`
2. `GET http://192-168-50-189.nip.io/api/health/summary`
3. `GET http://192-168-50-189.nip.io:30001/v1/models`
4. Probe the selected exact model alias through `POST /v1/responses`.

The inference API may allow anonymous LAN access or require a managed `gpu45_...`
key. On `401`, request a key from the owner; do not guess, scrape, or create one.

## Verified behavior

- Appliance version `0.1.0`, commit `a37b01181962ca54bdb5d295bbe3e4cf295d1522`.
- Health summary reported `ok`, with the inference proxy and backend ready.
- `/v1/models` returned twelve currently served aliases with explicit context-window
  metadata. Treat aliases and active models as dynamic and rediscover them rather
  than copying a model ID into source code.
- The primary inference surface is the streaming OpenAI Responses API at
  `POST /v1/responses`. As of the verification date it returns SSE even when a
  request explicitly sends `stream: false`; MCG's compatible adapter therefore
  accepts a terminal `response.completed` SSE event for buffered calls as a
  documented conformance exception. Buffered SSE is decoded incrementally with
  explicit per-event and total-response limits, and a valid terminal event
  settles immediately without depending on a later clean connection close.
- The current llama.cpp-compatible backend accepts
  `reasoning: {"effort":"none"}` per request. An uncontrolled tool turn emitted
  reasoning for minutes without reaching a tool call; the same control produced
  no reasoning events and a valid terminal completion. MCG records this behavior
  only through the explicit model probe and sends the control only to positively
  qualified tool-using routes.
- Local/OpenAI-compatible inference defaults to a five-minute response timeout
  because useful consumer tool turns on owner hardware can exceed the two-minute
  cloud default. Builder callers may still set a narrower explicit timeout, and
  timeout failures report the configured duration rather than a generic body
  decode error.
- The console also advertises management APIs for model control, benchmarks,
  transcription, speech, image/video/music generation, research, telemetry, and
  backup operations. Those are separate tool integrations and often require a
  management bearer token plus explicit authorization for mutations.

## Test policy

- Contract tests use a deterministic fake provider by default.
- LAN integration tests are opt-in and skip cleanly when GPU45 is unreachable.
- Live tests always rediscover models and select by declared capability/policy.
- Avoid parallel heavy GPU tests; the appliance serializes resource-intensive work.
- A timeout is ambiguous. Query existing jobs before retrying a job-creating call.
- Never include private prompts, access tokens, cookies, or generated private media
  in test logs or fixtures.

Run the opt-in live discovery and inference conformance test locally with:

```text
MCG_LIVE_GPU45=1 cargo test -p mcg-providers --test gpu45_live
```
