# Windows speech, audio and input options for callouts and push-to-talk

Research for the pitwall wayfinder ticket "Which Windows speech, audio and input options can pitwall use for callouts and push-to-talk?" ([#81](https://github.com/Vankata03/pitwall/issues/81)). Researched 2026-11-13 against Microsoft Learn, EA's help centre, the `windows` crate 0.62.2 source, and the source of the Rust crates named below.

This note records facts and trade-offs. It does not choose product behaviour; when alerts interrupt the driver belongs to [#59](https://github.com/Vankata03/pitwall/issues/59). Cloud provider facts (OpenAI and Gemini voice, push-to-talk semantics, session limits, prices) are already in the [AI feasibility note](https://github.com/Vankata03/pitwall/blob/research/ai-engineer-feasibility/docs/research/ai-engineer-feasibility.md) from [#58](https://github.com/Vankata03/pitwall/issues/58) and are only referenced here.

Every claim cites its source. **Unverified** marks claims that rest on community reports, inference or untested behaviour.

## Summary

- **Local speech works without a network or a paid API.** Windows 11 exposes two inbox speech engines to unpackaged desktop apps: the WinRT `SpeechSynthesizer` (OneCore voices) and SAPI 5 (`ISpVoice`, "Desktop" voices). Both are in the `windows` crate. On this machine (Ryzen 7 7800X3D), synthesising a 13-character callout took 2–24 ms and a 111-character callout 8–22 ms, measured to an in-memory WAV, playback excluded. The voices are the older, robotic-sounding ones (David, Zira, Mark for en-US). No first-party documentation says the newer Narrator "natural" voices are available to third-party apps, and community reports say they are not (**unverified**).
- **Cloud TTS changes the failure model rather than the speech code.** It adds network round-trips, which #58 left unmeasured, along with per-use cost and outage modes. A local engine is the only option that keeps speaking when the network drops.
- **Output device handling is the app's job if it uses WASAPI directly.** Microsoft documents that direct WASAPI clients must implement stream rerouting themselves. `cpal` 0.18 claims automatic rerouting of default-device streams on WASAPI, but its changelog contradicts itself on this, so it needs a test. A headset can be targeted through the `eCommunications` role or a user-chosen endpoint ID.
- **Ducking is opt-in through the stream type.** Windows attenuates other audio (80 % by default) only while a *communication stream* is open. Playing callouts as ordinary render streams, for example with the `AudioCategory_Speech` or `AudioCategory_GameChat` category, does not duck the game. Opening a communication stream to capture the microphone may duck the game unless the user has turned that off.
- **Input while the game has focus is the hard constraint.** Raw Input with `RIDEV_INPUTSINK` and DirectInput with `DISCL_BACKGROUND | DISCL_NONEXCLUSIVE` are documented to deliver input in the background. `RegisterHotKey` works system-wide but reports only the key press, not the release. Windows.Gaming.Input is documented as focus-only. XInput is documented as focus-gated on Windows 10 and later. GameInput supports background input only when its redistributable is installed and has no binding in the `windows` crate.
- **Anti-cheat.** None of these APIs inject into or read the game process; they read devices or receive OS messages in pitwall's own process. EA does not publish an allowlist. Its denylist is "not exhaustive" and targets drivers and conflicting software, and an EA forum post (second-hand; the page returned 403) attributes the reWASD block to virtual devices and a driver. Synthetic input (`SendInput`), virtual devices and kernel drivers carry the most risk. Passive reading carries the least, but no EA source confirms it is safe.

## 1. Local text-to-speech

### 1.1 Engines available on Windows 11

| | WinRT `Windows.Media.SpeechSynthesis.SpeechSynthesizer` | SAPI 5 (`ISpVoice` / `SpVoice`) |
| --- | --- | --- |
| Voices | OneCore voices, registered under `HKLM\SOFTWARE\Microsoft\Speech_OneCore\Voices\Tokens`. On this machine: Microsoft David, Zira, Mark (en-US) | "Desktop" voices under `HKLM\SOFTWARE\Microsoft\Speech\Voices\Tokens`. On this machine: David Desktop and Zira Desktop only (Mark is OneCore-only) |
| Restriction | "Only Microsoft-signed voices installed on the system can be used to generate speech." [1] | Any SAPI 5 engine registered on the system, including third-party engines |
| Input | Plain text or SSML 1.1 (`SynthesizeTextToStreamAsync`, `SynthesizeSsmlToStreamAsync`) [1] | Plain text, SAPI XML or W3C SSML (`SPF_PARSE_SSML`) [2] |
| Output | A `SpeechSynthesisStream` (WAV, `audio/wav`). The app must play it itself, for example with `MediaPlayer` or its own WASAPI stream [1] | Speaks to the default audio output, or to a stream or file set with `SetOutput` / `AudioOutputStream` |
| Interruption | The app stops its own playback. Synthesis returns whole streams | `SPF_ASYNC` queues and returns immediately; `SPF_PURGEBEFORESPEAK` "purges all pending speak requests prior to this speak call" [2] |
| Controls | `Options`: speaking rate, pitch, volume, appended silence (added in Windows 10 1703) [1] | Rate, volume, voice, via the interface or SAPI XML |
| `windows` crate feature | `Media_SpeechSynthesis` (plus `Media_Playback` and `Storage_Streams` to play it) | `Win32_Media_Speech` (`ISpVoice`, `SpVoice` CLSID) |
| Minimum OS | Windows 10 10240 [1] | Inbox since Vista. The SAPI 5.4 reference is archived [2] |

The registry listings above come from this machine, a Windows 11 Pro build 26300 install with only the en-US language pack. Other language packs add voices. Microsoft's Narrator appendix lists standard voices per language, for example George, Hazel and Susan for en-GB, which are added under *Speech settings > Manage voices* [3].

**Narrator natural voices.** Windows 11 Narrator offers downloadable "natural voices" for about ten languages and accents, including en-US, en-GB, en-IN, French, German and Spanish [3]. Microsoft's page does not say whether other apps can use them [3]. The open-source NaturalVoiceSAPIAdapter project says Microsoft "hasn't yet allowed third-party apps" to use them. It wraps them as SAPI engines unofficially and warns that this can break after a system update [4]. On this machine `AllVoices` returned only David, Zira and Mark, and no natural voice appeared. **Treat the natural voices as unavailable to pitwall (unverified, but consistent with every source found).**

### 1.2 Measured synthesis latency on this machine

This measurement was run with Windows PowerShell 5.1 calling the same WinRT and COM APIs the `windows` crate binds. Each engine synthesised two phrases three times into memory. Playback is not included.

| Engine | Construct | Phrase (13 chars), runs 1/2/3 | Phrase (111 chars), runs 1/2/3 | Audio produced (111 chars) |
| --- | --- | --- | --- | --- |
| WinRT `SpeechSynthesizer` | 206 ms | 24 / 2 / 2 ms | 22 / 8 / 8 ms | 257,486 bytes WAV |
| SAPI 5 `SpVoice` to `SpMemoryStream` | 31 ms | 8 / 2 / 2 ms | 12 / 11 / 11 ms | 354,708 bytes |

Interpretation:

- Synthesis is far faster than real time. The 111-character phrase is roughly eight seconds of speech, inferred from the byte counts at 16-bit mono and the engines' usual 16 kHz and 22.05 kHz formats (**format inferred, not read from headers**). The first call is slower and later calls are near-instant, so a long-lived synthesizer created at startup avoids the construction cost.
- Time to first audible sound also includes playback start-up. In shared mode, Windows 10 and later use a 10 ms engine buffer by default, and `IAudioClient3` can request smaller buffers if the driver supports them [5]. The added device and driver latency was not measured.
- This is one fast desktop CPU running nothing else. The game's CPU load, other hardware and other voices were not measured.

### 1.3 Calling from Rust

- **WinRT path.** With `windows` features `Media_SpeechSynthesis`, `Media_Playback`, `Storage_Streams` and `Foundation`, create a `SpeechSynthesizer` and call `SynthesizeTextToStreamAsync(...)?.get()?` (or await it). Either hand the stream to a `MediaPlayer`, or read the WAV bytes and play them through the app's own audio stream. `MediaPlayer.AudioDevice` accepts a `DeviceInformation` for a specific render endpoint [6]. The test above confirms the WinRT synthesizer works from an unpackaged desktop process.
- **SAPI path.** With `Win32_Media_Speech` and `Win32_System_Com`, `CoCreateInstance(&SpVoice, …)` yields an `ISpVoice`. `Speak(text, SPF_ASYNC | SPF_PURGEBEFORESPEAK, …)` covers fire-and-forget callouts with pre-emption [2].
- **The `tts` crate** (MIT) wraps WinRT on Windows. Its WinRT backend synthesises with `SynthesizeTextToStreamAsync` and plays through a `MediaPlayer` set to `MediaPlayerAudioCategory::Speech` [7]. The latest crates.io release is 0.26.3 (2024-07-06). The repository's main branch targets `windows` 0.62, but whether a release includes that is **unverified**.
- **Bundled neural TTS is a third option.** It avoids the dated inbox voices without using the cloud. One example is Piper, through `piper-rs` (MIT on crates.io). The original `rhasspy/piper` repository (MIT) is archived. Its maintained successor, `OHF-Voice/piper1-gpl`, is GPL-3.0, which matters for distribution. Each voice model carries its own licence (**not reviewed**). Model size, CPU cost alongside the game and quality were not evaluated.

## 2. Cloud TTS compared with local

The #58 note already records the provider options (OpenAI audio and Realtime, Gemini Live), push-to-talk and interruption semantics, session limits and prices. This section adds only the comparison.

| | Local (WinRT / SAPI / bundled model) | Cloud TTS |
| --- | --- | --- |
| Latency to first audio | Measured above: tens of milliseconds of synthesis plus device buffering | Network round-trip plus provider synthesis. Not measured in #58 or here |
| Network loss | Unaffected | Callout lost or delayed. Needs timeouts, and stale callouts must be dropped, as #58 proposed |
| Cost | None per use | Per character or per audio minute. See #58 for current rates |
| Quality | Older inbox voices sound robotic. Bundled neural models are better but unevaluated | Neural voices, generally higher quality (provider claim, not evaluated here) |
| Privacy | Nothing leaves the machine | Callout text is sent to the provider |
| Determinism | The same text gives the same audio | The same text normally gives the same voice. Retries and outages vary |

The trade-off is a design choice: whether a cloud voice may ever be the only path for a safety-relevant callout, or only an upgrade over a local fallback. That decision belongs to #59 and the owner.

## 3. Audio output: device choice, coexistence, ducking, device changes

### 3.1 Choosing and following a device

- Windows keeps default endpoints per role. *Console* and *multimedia* roles usually point at speakers. The **communications** role often points at a headset, but the user must choose it as the default communication device [8].
- An app can target the default device for a role (`IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender, eCommunications)`) or a specific endpoint ID chosen in its own settings [8]. `XInputGetAudioDeviceIds` returns the WASAPI render and capture IDs of a headset plugged into an Xbox controller [9].
- **Device changes.** High-level APIs (Media Foundation, DirectSound, WAVE) reroute streams on the default device automatically. "If your media application uses MMDeviceAPI or WASAPI directly, the application needs to provide the stream routing implementation." It does this by listening for default-device changes, device removal and format changes [10], through `IMMNotificationClient` (in the `windows` crate under `Win32_Media_Audio`).
- **`cpal`** (Apache-2.0, 0.18.2, 2026-08-16). Since 0.18.0, "Default output and input streams now automatically reroute when the system default device changes; stream error callback receives `ErrorKind::DeviceChanged`." The changelog then contradicts itself. 0.18.2 says WASAPI "Default device changes no longer report `DeviceChanged`, which wrongly implied the stream had rerouted automatically", and the unreleased section reverts that change "as it was misinformed" [11]. Treat automatic rerouting in released 0.18.x as **unverified until tested**. Streams opened on a *specific* device do not follow the default. When that device disappears, the app gets an error and must reopen (inferred from the changelog, which documents this for other hosts; WASAPI behaviour **unverified**). `rodio` (0.23.0, MIT/Apache-2.0) and `kira` (0.12.6) sit on top of cpal. The `wasapi` crate (0.27.0, MIT) is a thinner WASAPI wrapper for apps that want direct control.

### 3.2 Coexisting with the game and ducking

- Shared-mode render streams mix with the game's audio. Exclusive mode would stop other apps, the game included, from using that endpoint [5], so it is not an option here.
- **Ducking** is a Windows feature of *communication streams*: any stream opened on a communication device. While one is open, the default ducking setting lowers other streams by 80 %. The user can change the level, mute other streams instead, or turn ducking off on the Sound control panel's Communications tab [12][8].
- Stream categories adjust this. `AudioCategory_GameChat` is "Similar to `AudioCategory_Communications` except that `AudioCategory_GameChat` will not attenuate other streams". Capture streams may use only `Communications`, `Speech` or `Other` [13].
- `IAudioClientDuckingControl::SetDuckingOptionsForCurrentStream` lets a render stream opt out of ducking other streams. It needs Windows 10 build 20348 or later and affects only the stream it was obtained from [14]. It is in the `windows` crate under `Win32_Media_Audio`.
- If pitwall wants to *lower the game* while it speaks, the only documented mechanism is the system's communication-session ducking, which follows the user's setting [12]. Changing the game's own session volume through the audio session APIs would act on the game's audio session, not its process memory. Whether that is acceptable in spirit and under anti-cheat policy is a product decision and is **not researched here**.

## 4. Microphone capture for an optional voice conversation

- WASAPI capture in shared mode, through `cpal` input streams or the `wasapi` crate. The low-latency note says Windows 10 and later cut capture-side engine latency to about 0 ms, plus the driver buffer (10 ms by default) [5].
- A capture stream on the communications device counts as a communication stream and can trigger ducking of the game for the whole capture session [8][12]. Push-to-talk that opens capture only while the button is held limits how long ducking lasts. Keeping a stream open but discarding samples does not.
- Privacy: desktop apps need *Settings > Privacy & security > Microphone > Let desktop apps access your microphone* turned on [15]. Reports describe newer Windows 11 Insider builds adding per-app desktop prompts (**unverified**, secondary source). Capture should treat an access-denied failure as a user setting, not a crash.
- Speech recognition itself, whether cloud transcription or local whisper.cpp, is covered by #58.

## 5. Push-to-talk and "give me an update" input without touching the game

The game normally has focus while the driver races, so every option is judged on whether it delivers input to pitwall in the background.

| API | Devices | Works while the game has focus? | Press and release? | Notes |
| --- | --- | --- | --- | --- |
| `RegisterHotKey` | Keyboard key plus modifiers | Yes, system-wide hot key [16] | **Press only** (`WM_HOTKEY`). `MOD_NOREPEAT` suppresses auto-repeat [16] | Fails if another app already holds the combination. F12 is reserved [16]. Win-key combinations are reserved for the OS [16] |
| Raw Input (`RegisterRawInputDevices`) | Keyboard, mouse, any HID: wheels, button boxes, gamepads | Yes with `RIDEV_INPUTSINK`, which "enables the caller to receive the input even when the caller is not in the foreground"; needs `hwndTarget` [17] | Yes (key and button state changes) | Default is focus-only [17]. `RIDEV_DEVNOTIFY` gives hot-plug events [17]. HID reports must be parsed per device |
| Low-level keyboard hook (`WH_KEYBOARD_LL`) | Keyboard | Yes | Yes | Not injected: "the WH_KEYBOARD_LL hook is not injected into another process" [18]. Silently removed if it exceeds the timeout (at most 1 s since 1709) [18]. Microsoft recommends Raw Input instead [18] |
| DirectInput 8 | Wheels, joysticks, button boxes | Yes with `DISCL_BACKGROUND`. Use `DISCL_NONEXCLUSIVE` so the game keeps the device [19] | Yes (polled state) | Legacy, archived docs [19]. Force feedback needs exclusive access and is not wanted here |
| XInput 1.4 | Xbox-compatible controllers, up to 4 [9] | **Documented as no**: on Windows 10+, "game controller input is automatically enabled/disabled by the system based on the application window focus" [20] | Yes (polled) | Community reports of background XInput reads conflict with this (**unverified, needs a device test**). Most wheels are not XInput devices |
| Windows.Gaming.Input (`RacingWheel`, `RawGameController`, `Gamepad`) | Wheels, gamepads, generic controllers [21] | **No**: "A Windows application must have focus to receive input from a controller." [21] | Yes (polled readings) | `gilrs` uses this backend by default (feature `wgi`). Its `xinput` feature switches to XInput [22] |
| GameInput (Microsoft GDK, NuGet `Microsoft.GameInput`) | Keyboard, mouse, gamepads, wheels, raw HID [23] | Yes with `IGameInput::EnableBackgroundInput(true)` [24], but **only when the GameInput redistributable is installed**. With a sideloaded runtime the call "has no effect", and focus filtering is inconsistent [25] | Yes | Windows 10 19H1 and later [23]. **Not in the `windows` crate** (0.62.2 has no GameInput bindings), and no Rust binding crate was found. FFI would have to be written against `GameInput.h` |

Rust crates:

- **`global-hotkey`** (Tauri, Apache-2.0/MIT, 0.10.0). Uses `RegisterHotKey`. It synthesises a *Released* event by polling `GetAsyncKeyState` every 50 ms on a spawned thread after `WM_HOTKEY` [26]. Hold-to-talk therefore works, but release detection has up to about 50 ms of added latency.
- **`gilrs`** (0.11.2). Its default Windows backend is Windows.Gaming.Input, which is focus-only, so it is **not suitable as-is for background button reads** [21][22].
- **`windows` crate** features: `Win32_UI_Input_KeyboardAndMouse` (`RegisterHotKey`), `Win32_UI_Input` (`RegisterRawInputDevices`), `Win32_Devices_HumanInterfaceDevice` (`DirectInput8Create` and HID parsing), `Win32_UI_Input_XboxController` (`XInputGetState`, linked to `xinput1_4.dll`), `Gaming_Input`.
- pitwall's main app uses gpui. Raw Input needs a window to receive `WM_INPUT`. A dedicated input thread with its own hidden window and message loop keeps this independent of gpui's event loop. Whether a message-only window (`HWND_MESSAGE`) receives `RIDEV_INPUTSINK` input is **unverified**.

**Sharing a wheel button with the game.** Raw Input and non-exclusive DirectInput both read alongside the game; neither takes the device away. The game still sees the same button. If the button is also bound in F1 25, both act on it. Picking an unbound button is a user-setup concern.

## 6. Anti-cheat constraints (EA Javelin)

- EA describes Javelin as kernel-mode anti-cheat. It "will shut down the game and our services if you use cheat tools, debuggers, or other conflicting software". It keeps a denylist, the published list "isn't exhaustive", and the examples are mostly kernel drivers [27].
- EA forum guidance, in an EA post (the page returned 403 to the fetch tool; content via search summary, **second-hand**), attributes the reWASD block to its virtual devices and driver. EA community managers also call key remapping software, overlays and hardware monitors "more sensitive" programs [28]. Player-compiled lists (AutoHotkey, DS4Windows, JoyToKey and others) are not EA policy (**unverified**).
- What the APIs above do: each one receives OS messages or reads a device inside pitwall's own process. The low-level keyboard hook is documented as not injected into other processes [18]. None writes to the game, injects input or loads a driver.
- Patterns to avoid, because they match what EA's sources describe as conflicting: `SendInput` or `keybd_event` to press keys in the game, virtual HID or XInput devices (ViGEm-style), and any kernel driver. Exclusive DirectInput acquisition would also take the wheel from the game.
- EA does not say any input-reading API is permitted. Low risk here is an inference from how the APIs work and what EA lists, not an EA statement.

## Open uncertainties

1. Whether Windows 11 Narrator natural voices can be used by third-party apps. No first-party source; community sources say no.
2. Time to first audible audio for each engine, including `MediaPlayer` start-up and the WASAPI device buffer, on the owner's hardware with F1 25 running.
3. XInput background behaviour on current Windows 11 for an unfocused Win32 app. The documentation says focus-gated; community reports say otherwise.
4. Whether `RIDEV_INPUTSINK` works with a message-only window, and how the owner's specific wheel reports its buttons over HID.
5. Whether released `cpal` 0.18.x actually reroutes default-device WASAPI streams (its changelog contradicts itself), and what it does when a *specific*, non-default device is unplugged.
6. Any EA statement on passive input reading. None was found, and the EA forum post could not be fetched directly.
7. Voice-model licences and quality for bundled neural TTS (Piper and others).
8. Whether pitwall may lower the game's volume beyond system ducking. This is a product and policy question for #59.

## Sources

1. Microsoft Learn, *SpeechSynthesizer Class*. https://learn.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer
2. Microsoft Learn (archived), *SPEAKFLAGS (SAPI 5.4)*. https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee431843(v=vs.85)
3. Microsoft Support, *Appendix A: Supported languages and voices* (Narrator). https://support.microsoft.com/en-us/windows/appendix-a-supported-languages-and-voices-4486e345-7730-53da-fcfe-55cc64300f01
4. NaturalVoiceSAPIAdapter README (community). https://github.com/gexgd0419/NaturalVoiceSAPIAdapter
5. Microsoft Learn, *Low Latency Audio*. https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/low-latency-audio
6. Microsoft Learn, *MediaPlayer.AudioDevice Property*. https://learn.microsoft.com/en-us/uwp/api/windows.media.playback.mediaplayer.audiodevice
7. `tts-rs` source, `Cargo.toml` and `src/backends/winrt.rs`. https://github.com/ndarilek/tts-rs
8. Microsoft Learn, *Using a Communication Device*. https://learn.microsoft.com/en-us/windows/win32/coreaudio/using-the-communication-device
9. Microsoft Learn, *Getting Started With XInput*. https://learn.microsoft.com/en-us/windows/win32/xinput/getting-started-with-xinput
10. Microsoft Learn, *Stream Routing*. https://learn.microsoft.com/en-us/windows/win32/coreaudio/stream-routing
11. `cpal` CHANGELOG (0.18.0–0.18.2). https://github.com/RustAudio/cpal/blob/master/CHANGELOG.md
12. Microsoft Learn, *Default Ducking Experience*. https://learn.microsoft.com/en-us/windows/win32/coreaudio/stream-attenuation
13. Microsoft Learn, *AUDIO_STREAM_CATEGORY*. https://learn.microsoft.com/en-us/windows/win32/api/audiosessiontypes/ne-audiosessiontypes-audio_stream_category
14. Microsoft Learn, *IAudioClientDuckingControl*. https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iaudioclientduckingcontrol
15. Microsoft Support, *Turn on app permissions for your microphone in Windows*. https://support.microsoft.com/en-gb/windows/turn-on-app-permissions-for-your-microphone-in-windows-94991183-f69d-b4cf-4679-c98ca45f577a
16. Microsoft Learn, *RegisterHotKey function*. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey
17. Microsoft Learn, *RAWINPUTDEVICE* and *Raw Input Overview*. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinputdevice, https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input
18. Microsoft Learn, *LowLevelKeyboardProc*. https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
19. Microsoft Learn (archived), *IDirectInputDevice8::SetCooperativeLevel*. https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee417921(v=vs.85)
20. Microsoft Learn, *XInputEnable function*. https://learn.microsoft.com/en-us/windows/win32/api/xinput/nf-xinput-xinputenable
21. Microsoft Learn, *Windows.Gaming.Input Namespace*. https://learn.microsoft.com/en-us/uwp/api/windows.gaming.input
22. `gilrs-core` `Cargo.toml` (features `wgi` default, `xinput`). https://gitlab.com/gilrs-project/gilrs/-/blob/master/gilrs-core/Cargo.toml
23. Microsoft Learn, *GameInput introduction*. https://learn.microsoft.com/en-us/gaming/gdk/docs/features/common/input/overviews/input-overview
24. Microsoft Learn, *IGameInput::EnableBackgroundInput*. https://learn.microsoft.com/en-us/gaming/gdk/docs/reference/input/gameinput/interfaces/igameinput/methods/igameinput_enablebackgroundinput
25. Microsoft Learn, *GameInput sideloading*. https://learn.microsoft.com/en-us/gaming/gdk/docs/features/common/input/overviews/input-agility-redistributable
26. `global-hotkey` source, `src/platform_impl/windows/mod.rs`. https://github.com/tauri-apps/global-hotkey/blob/dev/src/platform_impl/windows/mod.rs
27. EA Help, *EA anticheat*. https://help.ea.com/en/help/pc/ea-anticheat/
28. EA Forums, *EA Javelin Anticheat & Recent Software Blocks* (second-hand; fetch returned 403). https://forums.ea.com/discussions/ea-forums-general-discussion-en/ea-javelin-anticheat--recent-software-blocks/12218073

Crate versions and licences come from the crates.io API on 2026-11-13. `windows` crate facts come from the 0.62.2 source (`Cargo.toml` features, `src/Windows/...`).
