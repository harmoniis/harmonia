# UI/UX — The Living Void

**"Thin Client, Fat Soul"** architecture. The mobile app is not the brain — it is a sensory organ and holographic projector for the agent living on the server. The interface is a canvas that the agent paints on via MQTT in real-time, using pre-built native template components.

---

## Visual Philosophy: "The Living Void"

Default state: **Zero UI.** Not a dashboard. Not a chat app. A waiting room for intent.

- **Aesthetic:** Ultra-minimalist, flat, high contrast. OLED Black `#000000` background. No persistent toolbars, hamburger menus, or navigation chrome.
- **The Heartbeat:** Center of screen — a subtle breathing abstract shape. Pulses slowly to indicate MQTT connection is alive. Changes behavior when thinking (color shift), speaking (waveform sync), or disconnected (dims).

---

## Interaction Model: Voice-First, Text-Ready

### Primary: "Shazam" Voice

- **Action:** Single tap on the Heartbeat activates microphone.
- **Feedback:** Shape morphs into waveform. Whisper runs locally — transcription appears as ghost text in real-time before audio is fully processed.
- **Response:** Agent response streamed via MQTT. TTS engine begins synthesizing immediately. Waveform syncs to agent's voice.

### Fallback: "Silent Mode" Text

- **Action:** Persistent minimalist text input at bottom of screen.
- **Behavior:** Tap brings keyboard. Heartbeat dims. Discrete communication without voice.

---

## The Infinite Timeline ("The Stream")

No sessions. No chat rooms. Only The Stream.

- **Vertical flow:** Bottom is "Now." Scroll up to see history.
- **Multimedia cards:** Not just text bubbles. A2UI template components mixed with voice logs, agent responses, and system events.
- **Search:** Pull down from top — semantic search queries the server.

---

## A2UI: Agent-Adaptive UI (Template Components)

The agent decides WHAT UI to render. The app contains pre-built native template components ("Dormant Modules") activated by MQTT render commands.

**Apple compliance:** No code is generated, compiled, interpreted, or downloaded at runtime. Every component is a native SwiftUI view / Jetpack Compose composable / Unity prefab that ships with the app binary. The agent selects and parameterizes existing templates.

### Workflow

1. **Agent Decision:** "User needs to see coffee shops and a button to navigate."
2. **MQTT Payload:** Agent publishes a `render_widget` command with component type and parameters.
3. **Client Rendering:** App parses the command and instantiates the pre-built native template component with the provided data.

### Template Component Library

| Template | Description |
|----------|-------------|
| `TextBubble` | Agent text response with optional audio playback |
| `VoiceWaveform` | Audio playback with animated waveform |
| `MapWithList` | Map view with POI markers and list |
| `MediaViewer` | Image/video display |
| `ListTable` | Data table with sortable columns |
| `ChoiceChips` | Quick reply buttons that fire MQTT events |
| `DeepLink` | Card that executes a system URL scheme |
| `FormInput` | Dynamic form fields with validation |
| `CodeBlock` | Syntax-highlighted code |
| `ProgressTracker` | Multi-step progress indicator |
| `Calendar` | Calendar view (day/week/month) |
| `Timer` | Countdown/countup timer |
| `PermissionCard` | Permission request with explanation |
| `WalletCard` | Webcash balance and transactions |
| `ContactCard` | Contact info with action buttons |
| `WeatherCard` | Weather display |
| `AudioPlayer` | Audio playback with scrubbing |
| `ImageGallery` | Swipeable image gallery |
| `Notification` | In-app notification banner |
| `Separator` | Visual divider |
| `Composite` | Container that nests other templates |

### XR-Exclusive Templates

| Template | Description |
|----------|-------------|
| `SpatialObject` | 3D model placed in space |
| `SpatialPanel` | 2D panel floating in 3D |
| `ChoiceOrbs` | Graspable spheres for selection |
| `SpatialMap` | 3D terrain miniature |
| `HolographicCode` | Code as floating hologram |

### Composition

The `Composite` template enables complex layouts by nesting other templates. The agent builds rich interfaces by combining simple templates — vertical stacks, horizontal rows, grids — without generating code.

---

## Navigation & Gestures

Zero UI means gesture-based navigation:

- **Tap Center:** Listen / Stop Listening
- **Swipe Right:** Settings & Connections (MQTT status, permissions, agent identity status)
- **Swipe Left:** "The Vault" (media/files the agent has saved)
- **Pull Down:** Search (semantic search of timeline history)

---

## The "Omniscient" Data Flow

For harmonic pattern detection, the app acts as a continuous data pump:

- **First launch:** App sequences permission requests. Each permission explained.
- **Background sync:**
  - GPS streamed via MQTT at intervals
  - Photos hashed and uploaded to object storage automatically
  - Clipboard monitored (iOS shows paste indicator; Android is silent)
  - Health data synced periodically
  - Motion/activity recognized and reported
  - Device state (battery, network, bluetooth) reported on change
  - Android-exclusive: SMS, call logs, all-app notifications read and forwarded
- **Agent-requested data:** Agent can request specific data via MQTT at any time (e.g., "take a photo," "read recent contacts," "get current heading")

---

## Technical Summary

| Aspect | Implementation |
|--------|----------------|
| Frontend | Swift (iOS SwiftUI) / Kotlin (Android Jetpack Compose) / C# (Unity XR) |
| State management | Reactive — UI is a direct function of MQTT stream |
| Latency handling | Optimistic UI. Ghost text appears immediately (local Whisper). Thinking animation until MQTT response arrives. |
| A2UI rendering | Pre-built native template components selected by MQTT render commands. No runtime code generation. |
| Deep linking | App dispatches ANY URL scheme available on the OS per agent instruction |
| Background | iOS: BGTaskScheduler + silent push. Android: Foreground service (START_STICKY). |
| Communication | All via harmoniislib (Rust C FFI). Platform code never touches JSON or MQTT directly. |
