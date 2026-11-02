# Optional AI engineer and session coach: feasibility

Researched 2026-11-02 for [Which AI approaches can support a grounded voice engineer and session coach?](https://github.com/Vankata03/pitwall/issues/58), under [pitwall expansion: driver experience, race advice and optional AI](https://github.com/Vankata03/pitwall/issues/54).

## Finding and evidence limits

Optional voice engineering and post-session coaching are technically plausible with current cloud APIs or a local speech/text pipeline. Deterministic callouts can provide useful spoken information without a language model. No provider, interaction style, hardware requirement or first-release scope is selected here.

This is documentation research and source inspection, not a performance evaluation. No actual telemetry or microphone audio was transmitted, credentials configured, services provisioned or paid inference performed. Provider descriptions of low latency are documented claims, not measured pitwall performance. Prices are public USD rates checked on the research date; account access, regional availability, taxes and future model availability remain unverified.

The approved boundary remains: deterministic calculations and alerts are authoritative; the core works without AI. AI can explain supplied evidence and propose app changes, but layout, settings and replay changes require explicit confirmation. Those constraints come from the parent map, not from a provider guarantee.

## Small comparison

| Approach | Documented building blocks | Practical fit and unresolved work |
| --- | --- | --- |
| Deterministic baseline | Fixed callout templates, recorded clips, or speech synthesis. Web Speech exposes local versus remote voices and cancellation. | Suitable for exact values and alert priorities. No conversational reasoning. Local voice availability and playback behavior need device checks. |
| Cloud speech/text pipeline | Speech transcription, a text model with function calling/structured outputs, then local speech or cloud TTS. | Separate transcription, evidence checks and speech are inspectable. More stages need cancellation and add latency; that cost is unmeasured. Text coaching can reuse the evidence format. |
| OpenAI native voice | Realtime speech-to-speech with tools, WebRTC in browsers and WebSocket on the host. GPT-Live is a distinct full-duplex voice frontend that delegates reasoning/tools. | Conversational option; native spoken answers are harder to validate before playback. Realtime and GPT-Live have different billing and session behavior. |
| Gemini native voice | Live API over WSS, audio input/output, transcripts and function calling. | Another cloud candidate with model-specific controls. The current stable model differs from legacy preview examples. |
| Local pipeline | whisper.cpp transcription, llama.cpp text inference/tools, then local speech synthesis. | Can operate offline after models are installed. Requires packaging, model licence review, cancellation and hardware evaluation alongside the game. |

The baseline should keep its wording deterministic even if the selected speech engine uses neural synthesis. The [Web Speech specification](https://webaudio.github.io/web-speech-api/#tts-section) defines `localService` and `cancel()`; browser speech is not automatically local. Bundled clips provide a separate candidate when a suitable local voice is unavailable. No paid API is necessary for that baseline; clip production and distribution rights still need review.

For a modular cloud example, [GPT-4o Mini Transcribe](https://developers.openai.com/api/docs/models/gpt-4o-mini-transcribe) supplies transcription, while [GPT-6 Luna](https://developers.openai.com/api/docs/models/gpt-6-luna) supports text input/output, function calling and structured outputs through Responses. It does not accept audio. A schema can enforce response shape, but cannot establish whether advice is true. Using a local voice avoids a second cloud audio service. Cloud TTS is also available through OpenAI's [audio APIs](https://developers.openai.com/api/docs/guides/audio), with additional usage and privacy implications.

[OpenAI Realtime](https://developers.openai.com/api/docs/guides/realtime) supports browser WebRTC and host WebSocket connections. The [Realtime 2.1 Mini model page](https://developers.openai.com/api/docs/models/gpt-realtime-2.1-mini) documents function calling and audio input/output, but no structured outputs. Its generic feature table says streaming is unsupported, while the Realtime guide documents continuous voice transport; this note relies on the transport guide for connection behavior and makes no stronger streaming feature claim. [GPT-Live 1](https://developers.openai.com/api/docs/models/gpt-live-1) separately documents simultaneous listening/speaking and backend delegation. These are candidate architectures, not interchangeable API names.

[Gemini 3.8 Live](https://ai.google.dev/gemini-api/docs/models/gemini-3.8-live) is listed as stable, with asynchronous function calling, audio responses and optional output transcription; structured outputs are unsupported. Proactive audio is permanently enabled, so muting and callout scheduling need application control. The generic tool guide still emphasizes older models; use the current model page for those differences. [Gemini 3.1 Flash Live Preview](https://ai.google.dev/gemini-api/docs/models/gemini-3.1-flash-live-preview) is now described as legacy, so old examples are not evidence of current defaults. Post-session written coaching should use a separate text request rather than assume the voice endpoint is a report generator.

## Interaction and integration constraints

### Existing boundary, verified from source

Source baseline: [`62960422f99d2afd7d39edeaa60a93c861395815`](https://github.com/Vankata03/pitwall/tree/62960422f99d2afd7d39edeaa60a93c861395815). The following are source findings, not a runtime audit:

- [`wire/mod.rs`](../../crates/collector/src/wire/mod.rs) publishes changed binary frames at up to 60 Hz, captures state every 200 ms, and publishes history changes separately. Frame/state queues coalesce; reliable history overflow disconnects a screen. [`wire/routes.rs`](../../crates/collector/src/wire/routes.rs) serves `/ws` and an epoch-qualified lap trace route. The stream is not an archive of every sample.
- [`runtime/stores.ts`](../../dashboard/src/runtime/stores.ts) separates frame rings from slower React state and rejects obsolete epochs. [`model/mod.rs`](../../crates/collector/src/model/mod.rs) distinguishes fresh, stale, restricted and absent readouts. An AI context must preserve those distinctions and never turn missing data into zero.
- [`replay/routes.rs`](../../crates/collector/src/replay/routes.rs) publishes either live or replay data while live ingest continues. Starting/stopping replay requires host access; pause, speed and seek controls permit LAN access. Coaching must bind evidence to the recording, source, epoch and replay position, rather than assume the displayed session is live.
- [`http/mod.rs`](../../crates/collector/src/http/mod.rs) and [`http/trust.rs`](../../crates/collector/src/http/trust.rs) separate LAN routes from actual loopback-peer host routes, with JSON/Origin/Host mutation checks. These checks are not user authentication. [`README.md`](../../README.md) documents no login or TLS and open LAN layout editing. Existing settings are not a credential store.
- [`runtime/client.ts`](../../dashboard/src/runtime/client.ts) reconnects the collector connection with bounded backoff and a silence watchdog. That is not cloud-session recovery or a freshness guarantee for spoken advice.

The [Media Capture specification](https://w3c.github.io/mediacapture-main/) restricts microphone access to secure contexts and requires permission. The [Secure Contexts specification](https://w3c.github.io/webappsec-secure-contexts/#is-origin-trustworthy) defines loopback exceptions. Inference: a host loopback page is a plausible microphone entry point; the ordinary plain-HTTP LAN URL cannot be assumed to support microphone capture on a tablet. A secure companion origin or host audio capture would require a separate design decision. User gesture, background audio and wheel/button input behavior remain device tests.

### Proposed boundary for later design

These are integration proposals, not implemented interfaces or accepted product decisions:

1. Keep AI work outside UDP ingest, deterministic calculation and dashboard rendering. A bounded worker receives selected summaries; a stalled cloud request must not hold model locks or delay recording.
2. Supply only the requested focus car, relevant competitors and time window. Include session/recording identity, epoch, car, units, timestamps, freshness, restrictions, calculation version and evidence IDs. Exclude names and raw packets unless specifically needed and later approved.
3. Offer narrow read tools such as a current race summary, a completed-lap comparison and a bounded trace window. The host calculates fuel margins, live delta and alert conditions. Search-engine grounding cannot substitute for these private session facts; neither live provider automatically sees the collector.
4. Require the response to distinguish observations, estimates and suggestions, and to cite evidence IDs that the dashboard can resolve. Reject unsupported numbers or stale evidence. Model instructions alone cannot enforce this; the application must validate outputs and tool arguments.
5. Treat any requested change as a proposal containing the exact action and affected object. Only a fresh, explicit user confirmation may invoke the existing mutation route, with its existing access checks. Spoken recognition of a confirmation needs separate design; an inferred acknowledgement is insufficient.

For coaching, first compute lap/stint summaries locally, then send only selected comparisons and trace excerpts. A report should retain evidence links into the chosen recording, identify invalid laps or missing channels, and separate observed differences from possible causes. A later approved evaluation should test whether claims about braking, tyres or setup are actually supported; a plausible explanation is not proof of causation.

### Interruption, push-to-talk and failure

OpenAI documents [push-to-talk and interruption](https://developers.openai.com/api/docs/guides/realtime-conversations): disable VAD, explicitly commit input and create a response; cancel generation and stop playback on a new turn, with truncation of unheard audio where required. Gemini documents [manual activity boundaries](https://ai.google.dev/gemini-api/docs/live-api/capabilities): disable automatic activity detection and send `activityStart`/`activityEnd`. Its [Live overview](https://ai.google.dev/gemini-api/docs/live-api) documents barge-in. Neither establishes performance with engine noise, game radio, speakers or this owner's microphone.

Inference: push-to-talk could reduce accidental transmission and false turn detection, but imposes an input burden that the owner has not accepted. An open microphone would need visible capture state and a hard mute. All approaches need one audio scheduler so a deterministic urgent alert can stop optional speech, discard expired callouts, and avoid several screens speaking at once. The owner still decides priorities, verbosity and interruption tolerance.

On cloud failure, the proposed behavior is to preserve deterministic display/callouts, mark AI unavailable, cancel pending speech, discard delayed live advice and reconnect with fresh context. Use bounded retries, session and spend limits, and avoid automatic provider switching that changes privacy terms. Post-session jobs can be retried explicitly with duplicate-request controls; interrupted requests may already be billable. This failure policy is a proposal, not existing behavior.

Gemini's [session management guide](https://ai.google.dev/gemini-api/docs/live-api/session-management) documents time limits, compression, resumption and `GoAway`. It describes roughly ten-minute connections and fifteen-minute audio sessions without compression. Confirm those limits for the selected model before implementation; compression or resumption does not prove recovery after an arbitrary network outage. OpenAI's [cost guide](https://developers.openai.com/api/docs/guides/voice-latency-cost) explains accumulating conversation costs and the need to close idle GPT-Live sessions; muting does not close them.

## Privacy, retention and keys

OpenAI says API content is not used for training by default. Abuse monitoring can retain content up to 30 days, with stated exceptions; Zero Data Retention/Modified Abuse Monitoring require eligibility and approval. Responses has default application-state retention; an eventual implementation should request `store:false` and inspect model-specific caching and endpoint exceptions. Disabling storage does not eliminate abuse logs or every cache. GPT-Live session recording storage is disabled by default; backend calls have separate controls. Realtime tracing is not currently EU data-residency compliant. These are [documented controls](https://developers.openai.com/api/docs/guides/your-data), not an account configuration verified here.

Google's [terms](https://ai.google.dev/gemini-api/terms) distinguish paid services from unpaid services: paid prompts/responses are not used to improve products; unpaid use has different treatment, with an EEA/Switzerland/UK exception. Do not assume a free tier is private or infer the owner's applicable region. The [abuse-monitoring policy](https://ai.google.dev/gemini-api/docs/usage-policies) describes 55-day retention of prompts, contextual information and output, with controlled human review. No blanket zero-retention or audio-specific deletion guarantee was established. Search grounding has separate retention terms and is unnecessary for session facts.

OpenAI provides [short-lived client secrets](https://developers.openai.com/api/reference/resources/realtime/subresources/client_secrets/methods/create); Gemini provides [ephemeral tokens](https://ai.google.dev/gemini-api/docs/live-api/ephemeral-tokens). These reduce exposure of the long-lived owner key, but remain usable credentials. Expiration is not a universal session-duration or spending cap.

Proposed key handling: keep the long-lived key on the host, preferably in an OS credential facility, and never embed it in dashboard assets, browser storage, layouts, exported recordings, URLs or logs. Restrict token issuance and paid requests to an authorized session; an unrestricted LAN token endpoint could spend the owner's balance. Require an explicit cloud opt-in with a preview of selected fields, microphone state, retention terms and a stop control. Persist neither transcripts nor provider traces by default. Local processing can avoid provider disclosure only if all selected components are actually local, outbound calls are disabled, and logs/model downloads are accounted for. These controls need design and testing; no credentials were inspected or changed.

## Prices and transparent examples

Rates below are uncached public rates on 2026-11-02. Per-million-token rates are input/output. These are illustrative components, not quality-equivalent configurations.

| Component | Rate and source |
| --- | --- |
| Deterministic clips/local voice | No cloud inference charge; device, storage and authoring costs remain. |
| OpenAI Realtime 2.1 Mini | Audio $10/$20; text $0.60/$2.40. Full Realtime 2.1: audio $32/$64, text $4/$24. [Pricing](https://developers.openai.com/api/docs/pricing). |
| GPT-Live 1 | $0.05 per active session minute, billed per second, plus backend usage. [Model](https://developers.openai.com/api/docs/models/gpt-live-1). |
| Modular transcription | GPT-4o Mini Transcribe estimated $0.003/audio minute. [Pricing](https://developers.openai.com/api/docs/pricing). |
| Modular text/coaching | GPT-6 Luna Standard $0.10/$0.50, short context. [Model rates and conditions](https://developers.openai.com/api/docs/models/gpt-6-luna). |
| Gemini 3.8 Live | Audio $3/$12; text $0.75/$4.50. [Pricing](https://ai.google.dev/gemini-api/docs/pricing). |
| Local pipeline | No hosted inference bill; electricity, hardware, storage and maintenance are not free. |

Example A assumes a one-hour driving session with ten minutes of submitted speech. The modular pipeline uses 60,000 total billed text-input tokens and 10,000 output tokens across all turns, including repeated evidence/tool context and any billable reasoning. Local speech output adds no API fee. Arithmetic: transcription `10 * $0.003 = $0.030`; text `(60,000 * $0.10 + 10,000 * $0.50) / 1,000,000 = $0.011`; total **$0.041**. This is a workload assumption, not a measured token count or a promise that this model is a sufficient coach.

Example B assumes total billed usage of 12,000 audio-input, 24,000 audio-output, 10,000 text-input and 2,000 text-output tokens. Realtime Mini costs `(12,000*10 + 24,000*20 + 10,000*0.60 + 2,000*2.40)/1,000,000 = $0.6108`; full Realtime costs **$2.008**. At Gemini's listed rates, the same numerical token totals cost **$0.3405**. Tokens are provider-specific: this is an arithmetic sensitivity example, not equal audio duration, equal quality or an hour-of-driving estimate. Actual voice duration, history reuse, silence handling and tool results need metering.

Example C: a GPT-Live connection open for 60 minutes costs `60 * $0.05 = $3.00`, including idle connected time, plus backend work. With Example A's text totals, that is **$3.011**. WebRTC initialization has a 15-second charge credited against a running session; failed/repeated starts can change the bill. See the [cost guide](https://developers.openai.com/api/docs/guides/voice-latency-cost).

A separate written coaching request with 20,000 billed input and 2,000 output tokens costs `(20,000*0.10 + 2,000*0.50)/1,000,000 = $0.003` at the stated text rate. Longer recordings may need several requests. All examples exclude tax, regional premiums, retries, hosted search/storage/tools and cloud TTS. Assume no free credits or cache discounts. A future cost ceiling needs application enforcement and measured usage; public unit prices alone cannot bound a session bill.

## Local hardware and decisions still open

[whisper.cpp](https://github.com/ggml-org/whisper.cpp) documents CPU operation, acceleration backends and quantization. Its model table estimates base at 142 MiB disk/~388 MB memory and small at 466 MiB/~852 MB. Those are transcription footprints, not total application memory or acceptable transcription speed. Its streaming examples demonstrate building blocks, not pitwall's latency or noise tolerance.

[llama.cpp](https://github.com/ggml-org/llama.cpp) supports local inference with quantization and CPU/GPU backends; its [function-calling guide](https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md) documents `llama-server --jinja`, model templates and format-dependent behavior. A small tool-capable instruct model, such as a supported Qwen-family model, is a research candidate rather than a chosen dependency. Model weights have their own terms; engine licensing does not grant rights to every model or voice.

Memory inference: an 8-billion-parameter model at an idealized 4 bits/weight alone is about 4 GB decimal before metadata, quantization overhead, KV cache and runtime buffers. That is not a 4 GB hardware requirement. Context length, concurrent speech/text models, GPU offload, OS memory, and the running game's VRAM/CPU demand determine usable headroom. No local model was downloaded or benchmarked, and the owner's hardware is unknown. Post-session work can tolerate waiting and release game resources; live use must prove it does not harm driving performance.

The owner still needs to decide data fields and retention, cloud/local preference, cost ceiling, microphone device and push-to-talk tolerance, acceptable answer delay, audio priorities, coaching questions and confirmation behavior. These remain human-in-the-loop decisions.

Before any implementation commitment, a separately approved evaluation should use synthetic fixtures first, then only explicitly authorized recordings/audio. Measure end-of-speech to first useful audio, tool round-trip delay, stale-answer rate, numeric/evidence accuracy, interruption stop delay, false activations, network recovery, actual billed usage, and game frame-time/collector drops. Report distributions and failures under game noise and network loss. This investigation measured none of those outcomes and does not establish real-time reliability.
