# A2UI — Agent-Adaptive UI Specification

**Version 1.0 — Platform-Agnostic Component Contract**

This document defines the canonical A2UI component set, MQTT render protocol, and platform identification system. Both iOS and Android apps implement these components identically. The agent sends a single render command format; platform-specific code maps it to native views.

---

## Principles

1. **No runtime code generation.** Every component ships pre-compiled in the app binary.
2. **Backward compatible.** Use only basic, widely-supported UI primitives. No platform-specific flourishes. Components must render correctly on:
   - iOS 15.0+ (SwiftUI, covers ~97% active devices)
   - Android API 26+ / Android 8.0 Oreo (Jetpack Compose, covers ~95% active devices)
3. **Composable.** Complex layouts are built by nesting simple components via `Composite`.
4. **Platform-transparent to the agent.** The agent knows *which* platform it's talking to (via MQTT headers) but sends the same render commands regardless. Platform-specific capabilities (SMS, accessibility) use separate MQTT topics, not A2UI.

---

## MQTT Platform Identification

### Connection Handshake

When a client connects, it publishes a `connect` message on its device topic:

**Topic:** `harmonia/{agent_id}/device/{device_id}/connect`

**Payload (JSON on wire, sexp in Lisp):**

```json
{
  "platform": "ios",
  "platform_version": "17.2",
  "app_version": "1.0.0",
  "device_id": "UUID-...",
  "device_model": "iPhone 15 Pro",
  "screen_width": 393,
  "screen_height": 852,
  "screen_scale": 3.0,
  "locale": "en_US",
  "timezone": "America/New_York",
  "capabilities": [
    "voice", "location", "camera", "contacts", "calendar",
    "health", "bluetooth", "nfc", "haptics", "push",
    "background_location", "photo_library"
  ],
  "permissions_granted": [
    "microphone", "location_always", "camera", "contacts",
    "calendar", "photos", "notifications", "bluetooth"
  ],
  "permissions_denied": ["health", "motion"],
  "a2ui_version": "1.0"
}
```

**Platform values:** `"ios"`, `"android"`, `"xr"` (future)

### Lisp-Side Platform Registry

```lisp
;; orchestrator/platform.lisp

(defvar *connected-devices* (make-hash-table :test 'equal)
  "Registry of connected client devices, keyed by device-id.")

(defstruct device-info
  device-id
  platform         ; :ios, :android, :xr
  platform-version
  app-version
  model
  screen-width
  screen-height
  capabilities     ; list of keywords
  permissions      ; alist of (permission . :granted/:denied)
  connected-at
  last-seen)

(defun on-device-connect (connect-data)
  "Handle client connection. Register device, adjust behavior."
  (let* ((platform (intern (string-upcase (getf connect-data :platform)) :keyword))
         (device (make-device-info
                  :device-id (getf connect-data :device-id)
                  :platform platform
                  :platform-version (getf connect-data :platform-version)
                  :app-version (getf connect-data :app-version)
                  :model (getf connect-data :device-model)
                  :screen-width (getf connect-data :screen-width)
                  :screen-height (getf connect-data :screen-height)
                  :capabilities (mapcar (lambda (c) (intern (string-upcase c) :keyword))
                                        (getf connect-data :capabilities))
                  :permissions (build-permission-alist connect-data)
                  :connected-at (get-universal-time)
                  :last-seen (get-universal-time))))
    (setf (gethash (getf connect-data :device-id) *connected-devices*) device)
    (log-event :device-connect device)))

(defun device-can-p (device-id capability)
  "Check if a connected device has a specific capability."
  (let ((device (gethash device-id *connected-devices*)))
    (and device (member capability (device-info-capabilities device)))))

(defun device-platform (device-id)
  "Get the platform of a connected device."
  (let ((device (gethash device-id *connected-devices*)))
    (when device (device-info-platform device))))

(defun android-devices ()
  "Return list of connected Android devices."
  (loop for device being the hash-values of *connected-devices*
        when (eq (device-info-platform device) :android)
        collect device))

(defun ios-devices ()
  "Return list of connected iOS devices."
  (loop for device being the hash-values of *connected-devices*
        when (eq (device-info-platform device) :ios)
        collect device))
```

### Platform-Specific MQTT Topics

The agent uses platform-specific topics only for capabilities that differ between platforms:

```
harmonia/{agent_id}/cmd/{device_id}/render    — A2UI render (same for all platforms)
harmonia/{agent_id}/cmd/{device_id}/tts       — Text-to-speech (same for all)
harmonia/{agent_id}/cmd/{device_id}/deeplink  — Deep link (same format, platform resolves)

harmonia/{agent_id}/cmd/{device_id}/sms       — Send SMS (Android only, ignored by iOS)
harmonia/{agent_id}/cmd/{device_id}/call      — Make call (Android: direct, iOS: dialer only)
harmonia/{agent_id}/cmd/{device_id}/overlay   — Draw overlay (Android only)
harmonia/{agent_id}/cmd/{device_id}/settings  — Modify system settings (Android only)
harmonia/{agent_id}/cmd/{device_id}/tap       — Accessibility auto-tap (Android only)
```

The agent checks `(device-platform device-id)` before sending platform-specific commands.

---

## Render Command Format

Every render command follows this structure:

**MQTT Topic:** `harmonia/{agent_id}/cmd/{device_id}/render`

**Payload:**

```json
{
  "id": "widget-uuid-123",
  "component": "TextBubble",
  "data": { ... component-specific fields ... },
  "position": "timeline",
  "priority": "normal",
  "replace_id": null,
  "expires_at": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique widget instance ID |
| `component` | string | Component name (from the table below) |
| `data` | object | Component-specific parameters |
| `position` | string | `"timeline"` (append to stream), `"overlay"` (float above), `"replace"` (replace existing) |
| `priority` | string | `"low"`, `"normal"`, `"high"`, `"urgent"` |
| `replace_id` | string? | If set, replaces widget with this ID |
| `expires_at` | string? | ISO 8601 timestamp. Widget auto-removes after this time. |

---

## Component Registry

### Design Constraints (Backward Compatibility)

Every component uses ONLY these primitives, available on both iOS 15+ and Android API 26+:

| Primitive | iOS (SwiftUI) | Android (Compose) |
|-----------|---------------|-------------------|
| Vertical stack | `VStack` | `Column` |
| Horizontal stack | `HStack` | `Row` |
| Scroll | `ScrollView` | `LazyColumn` / `verticalScroll` |
| Text | `Text` | `Text` |
| Image (URL) | `AsyncImage` (iOS 15+) | `AsyncImage` (Coil) |
| Button | `Button` | `Button` / `TextButton` |
| Text field | `TextField` | `TextField` / `OutlinedTextField` |
| Toggle | `Toggle` | `Switch` |
| Slider | `Slider` | `Slider` |
| Progress bar | `ProgressView` | `LinearProgressIndicator` |
| Card/surface | `RoundedRectangle` + `background` | `Card` / `Surface` |
| Divider | `Divider` | `Divider` |
| Spacer | `Spacer` | `Spacer` |
| Map | `Map` (MapKit, iOS 17) / `MKMapView` (UIKit) | Google Maps Compose |
| Grid | `LazyVGrid` (iOS 14+) | `LazyVerticalGrid` |

**Forbidden:** NavigationStack (iOS 16+), Material3 BottomSheet (unstable), Sheets, NavigationDrawer, custom renderers, WebView-based components, canvas-drawn components. Keep it simple.

---

### Component Specifications

#### 1. TextBubble

Agent text response with optional audio playback button.

```json
{
  "component": "TextBubble",
  "data": {
    "text": "The weather in Berlin is 12°C and cloudy.",
    "audio_url": "https://..../response.mp3",
    "markdown": false
  }
}
```

| Field | Type | Required | Default |
|-------|------|----------|---------|
| `text` | string | yes | — |
| `audio_url` | string? | no | null |
| `markdown` | bool | no | false |

**iOS:** `Text` + optional play `Button`. **Android:** `Text` + optional play `IconButton`.

#### 2. VoiceWaveform

Audio playback with waveform visualization.

```json
{
  "component": "VoiceWaveform",
  "data": {
    "audio_url": "https://..../voice.mp3",
    "duration_ms": 4500,
    "waveform_data": [0.1, 0.3, 0.8, 0.5, 0.2]
  }
}
```

| Field | Type | Required |
|-------|------|----------|
| `audio_url` | string | yes |
| `duration_ms` | int | yes |
| `waveform_data` | float[]? | no |

#### 3. MapWithList

Map view with POI markers and a scrollable list below.

```json
{
  "component": "MapWithList",
  "data": {
    "center_lat": 52.52,
    "center_lon": 13.405,
    "zoom": 14,
    "points": [
      {"lat": 52.521, "lon": 13.410, "label": "Coffee House", "snippet": "0.3 km", "action_url": "maps://?daddr=52.521,13.410"},
      {"lat": 52.519, "lon": 13.401, "label": "Cafe Berlin", "snippet": "0.5 km", "action_url": "maps://?daddr=52.519,13.401"}
    ],
    "list_action_label": "Navigate"
  }
}
```

| Field | Type | Required |
|-------|------|----------|
| `center_lat` | float | yes |
| `center_lon` | float | yes |
| `zoom` | int | no (default 14) |
| `points` | object[] | yes |
| `points[].lat` | float | yes |
| `points[].lon` | float | yes |
| `points[].label` | string | yes |
| `points[].snippet` | string? | no |
| `points[].action_url` | string? | no |
| `list_action_label` | string? | no |

#### 4. MediaViewer

Display an image or video.

```json
{
  "component": "MediaViewer",
  "data": {
    "url": "https://..../photo.jpg",
    "media_type": "image",
    "caption": "Sunset at the lake",
    "aspect_ratio": 1.5
  }
}
```

| Field | Type | Required |
|-------|------|----------|
| `url` | string | yes |
| `media_type` | `"image"` / `"video"` | yes |
| `caption` | string? | no |
| `aspect_ratio` | float? | no (default 16:9) |

#### 5. ListTable

Data table with optional sorting and row actions.

```json
{
  "component": "ListTable",
  "data": {
    "headers": ["Name", "Price", "Rating"],
    "rows": [
      ["Coffee House", "$4.50", "4.8"],
      ["Cafe Berlin", "$3.80", "4.5"]
    ],
    "sortable": true,
    "row_actions": [
      {"label": "Navigate", "action_topic": "harmonia/cmd/navigate"}
    ]
  }
}
```

| Field | Type | Required |
|-------|------|----------|
| `headers` | string[] | yes |
| `rows` | string[][] | yes |
| `sortable` | bool | no (default false) |
| `row_actions` | object[]? | no |

#### 6. ChoiceChips

Quick-reply buttons. User taps one, MQTT event fires.

```json
{
  "component": "ChoiceChips",
  "data": {
    "choices": [
      {"label": "Yes", "value": "yes"},
      {"label": "No", "value": "no"},
      {"label": "Maybe later", "value": "later"}
    ],
    "callback_topic": "harmonia/{agent_id}/response/choice",
    "multi_select": false
  }
}
```

| Field | Type | Required |
|-------|------|----------|
| `choices` | object[] | yes |
| `choices[].label` | string | yes |
| `choices[].value` | string | yes |
| `choices[].icon` | string? | no |
| `callback_topic` | string | yes |
| `multi_select` | bool | no (default false) |

#### 7. DeepLink

Card that opens a URL scheme when tapped.

```json
{
  "component": "DeepLink",
  "data": {
    "label": "Open Bluetooth Settings",
    "url": "prefs:root=Bluetooth",
    "icon": "bluetooth",
    "description": "Turn on Bluetooth to connect your headphones"
  }
}
```

#### 8. FormInput

Dynamic form with fields and validation.

```json
{
  "component": "FormInput",
  "data": {
    "fields": [
      {"name": "name", "label": "Your Name", "type": "text", "required": true},
      {"name": "email", "label": "Email", "type": "email", "required": true},
      {"name": "notes", "label": "Notes", "type": "textarea", "required": false},
      {"name": "priority", "label": "Priority", "type": "select",
       "options": ["Low", "Normal", "High"]}
    ],
    "submit_label": "Submit",
    "submit_topic": "harmonia/{agent_id}/response/form"
  }
}
```

**Field types:** `"text"`, `"email"`, `"number"`, `"phone"`, `"textarea"`, `"select"`, `"toggle"`, `"date"`, `"time"`, `"slider"`.

All field types map to basic OS-provided input widgets — no custom renderers.

#### 9. CodeBlock

Syntax-highlighted code display.

```json
{
  "component": "CodeBlock",
  "data": {
    "language": "python",
    "code": "def hello():\n    print('Hello, World!')",
    "line_numbers": true,
    "copyable": true
  }
}
```

#### 10. ProgressTracker

Multi-step progress indicator.

```json
{
  "component": "ProgressTracker",
  "data": {
    "title": "Booking Flight",
    "steps": ["Search", "Compare", "Book", "Confirm"],
    "current_step": 2
  }
}
```

#### 11. Calendar

Calendar view with events.

```json
{
  "component": "Calendar",
  "data": {
    "view_mode": "week",
    "events": [
      {"title": "Meeting with John", "start": "2026-02-17T10:00:00Z",
       "end": "2026-02-17T11:00:00Z", "color": "#4A90D9"}
    ],
    "action_topic": "harmonia/{agent_id}/response/calendar_tap"
  }
}
```

**View modes:** `"day"`, `"week"`, `"month"`.

#### 12. Timer

Countdown or countup timer.

```json
{
  "component": "Timer",
  "data": {
    "duration_ms": 300000,
    "label": "Meditation",
    "auto_start": true,
    "callback_topic": "harmonia/{agent_id}/response/timer_done"
  }
}
```

#### 13. PermissionCard

Explains why a permission is needed, with a grant button.

```json
{
  "component": "PermissionCard",
  "data": {
    "permission_type": "location_always",
    "reason": "I need continuous location access to learn your daily patterns and proactively suggest route optimizations.",
    "grant_topic": "harmonia/{agent_id}/response/grant_permission"
  }
}
```

#### 14. WalletCard

Webcash balance and recent transactions.

```json
{
  "component": "WalletCard",
  "data": {
    "balance": "2.50000000",
    "currency": "webcash",
    "transactions": [
      {"type": "received", "amount": "1.00000000", "timestamp": "2026-02-17T09:00:00Z", "memo": "Payment from Alice"}
    ],
    "action_topic": "harmonia/{agent_id}/response/wallet_action"
  }
}
```

#### 15. ContactCard

Contact info with action buttons.

```json
{
  "component": "ContactCard",
  "data": {
    "name": "John Doe",
    "phone": "+49123456789",
    "email": "john@example.com",
    "avatar_url": "https://.../avatar.jpg",
    "actions": [
      {"label": "Call", "action": "tel:+49123456789"},
      {"label": "Message", "action": "sms:+49123456789"}
    ]
  }
}
```

#### 16. WeatherCard

Weather display.

```json
{
  "component": "WeatherCard",
  "data": {
    "location": "Berlin",
    "current": {"temp_c": 12, "condition": "cloudy", "humidity": 65, "wind_kmh": 15},
    "forecast": [
      {"day": "Tue", "high_c": 14, "low_c": 8, "condition": "partly_cloudy"},
      {"day": "Wed", "high_c": 16, "low_c": 9, "condition": "sunny"}
    ]
  }
}
```

**Conditions:** `"sunny"`, `"partly_cloudy"`, `"cloudy"`, `"rain"`, `"thunderstorm"`, `"snow"`, `"fog"`, `"wind"`. Mapped to SF Symbols (iOS) / Material Icons (Android).

#### 17. AudioPlayer

Full audio player with scrubbing.

```json
{
  "component": "AudioPlayer",
  "data": {
    "url": "https://.../podcast.mp3",
    "title": "Episode 42",
    "waveform_data": [0.1, 0.3, 0.8, 0.5, 0.2]
  }
}
```

#### 18. ImageGallery

Swipeable image gallery.

```json
{
  "component": "ImageGallery",
  "data": {
    "images": [
      {"url": "https://.../1.jpg", "caption": "First"},
      {"url": "https://.../2.jpg", "caption": "Second"}
    ]
  }
}
```

#### 19. Notification

In-app notification banner (not OS-level push).

```json
{
  "component": "Notification",
  "data": {
    "title": "Meeting in 10 minutes",
    "body": "With John at Cafe Roma",
    "priority": "high",
    "actions": [
      {"label": "Navigate", "action_url": "maps://?daddr=52.52,13.405"},
      {"label": "Dismiss", "action": "dismiss"}
    ],
    "auto_dismiss_ms": 10000
  }
}
```

**Priority levels:** `"low"` (subtle), `"normal"` (standard), `"high"` (prominent), `"urgent"` (full-width, vibrate).

#### 20. Separator

Visual divider with optional label.

```json
{
  "component": "Separator",
  "data": {
    "style": "line",
    "label": "Earlier today"
  }
}
```

**Styles:** `"line"`, `"space"`, `"dot"`.

#### 21. Composite

Container that nests other components. This is how complex layouts are built.

```json
{
  "component": "Composite",
  "data": {
    "layout": "vertical",
    "spacing": 8,
    "children": [
      {"id": "w1", "component": "TextBubble", "data": {"text": "Here are some coffee shops:"}},
      {"id": "w2", "component": "MapWithList", "data": {"center_lat": 52.52, "center_lon": 13.405, "points": [...]}},
      {"id": "w3", "component": "ChoiceChips", "data": {"choices": [{"label": "Navigate", "value": "go"}], "callback_topic": "..."}}
    ]
  }
}
```

**Layout modes:** `"vertical"`, `"horizontal"`, `"grid"`.

---

## Component Summary Table

| # | Component | Primitives Used | Backward Safe |
|---|-----------|----------------|---------------|
| 1 | TextBubble | Text, Button | iOS 15+, API 26+ |
| 2 | VoiceWaveform | Canvas, Slider, Button | iOS 15+, API 26+ |
| 3 | MapWithList | Map, List, Button | iOS 15+, API 26+ |
| 4 | MediaViewer | AsyncImage, VideoPlayer | iOS 15+, API 26+ |
| 5 | ListTable | Grid/Table, Text, Button | iOS 15+, API 26+ |
| 6 | ChoiceChips | HStack/Row, Button | iOS 15+, API 26+ |
| 7 | DeepLink | Card, Text, Button | iOS 15+, API 26+ |
| 8 | FormInput | TextField, Toggle, Picker | iOS 15+, API 26+ |
| 9 | CodeBlock | ScrollView, Text (monospace) | iOS 15+, API 26+ |
| 10 | ProgressTracker | HStack/Row, Circle, Text | iOS 15+, API 26+ |
| 11 | Calendar | Grid, Text, Scroll | iOS 15+, API 26+ |
| 12 | Timer | Text, ProgressView, Button | iOS 15+, API 26+ |
| 13 | PermissionCard | Card, Text, Button | iOS 15+, API 26+ |
| 14 | WalletCard | Card, Text, List | iOS 15+, API 26+ |
| 15 | ContactCard | Card, Image, Text, Button | iOS 15+, API 26+ |
| 16 | WeatherCard | Card, Image, Text | iOS 15+, API 26+ |
| 17 | AudioPlayer | Slider, Button, Canvas | iOS 15+, API 26+ |
| 18 | ImageGallery | Pager/TabView, AsyncImage | iOS 15+, API 26+ |
| 19 | Notification | Card, Text, Button, anim | iOS 15+, API 26+ |
| 20 | Separator | Divider, Text | iOS 15+, API 26+ |
| 21 | Composite | VStack/Column, HStack/Row, Grid | iOS 15+, API 26+ |

---

## XR Components

**Status: TODO.** XR (Unity/Oculus/CloudXR) components are deferred to Phase 2. The following are reserved component names that will be defined later:

- `SpatialObject` — 3D model placed in space
- `SpatialPanel` — 2D panel floating in 3D
- `ChoiceOrbs` — Graspable spheres for selection
- `SpatialMap` — 3D terrain miniature
- `HolographicCode` — Code as floating hologram

XR clients will connect with `"platform": "xr"` and the agent will check `(device-platform device-id)` before sending XR-specific commands.

---

## Platform Capability Differences (Agent Reference)

When the agent orchestrates, it must check capabilities. Reference table:

| Capability | iOS | Android | MQTT Topic |
|------------|-----|---------|------------|
| A2UI render | All 21 components | All 21 components | `.../render` |
| Voice (STT/TTS) | Whisper + AVSpeech | Whisper.cpp + Android TTS | `.../tts` |
| Deep links | `prefs:root=...` format | `android.settings.X` format | `.../deeplink` |
| SMS read/send | **BLOCKED** | Full access | `.../sms` |
| Call (direct) | Dialer only | Direct call | `.../call` |
| Notification read | **BLOCKED** | NotificationListenerService | sensor topic |
| Screen read/tap | **BLOCKED** | AccessibilityService | `.../tap` |
| System settings | **BLOCKED** | WRITE_SETTINGS | `.../settings` |
| Draw overlay | **BLOCKED** | SYSTEM_ALERT_WINDOW | `.../overlay` |
| Background | BGTaskScheduler (30s) | Foreground service (unlimited) | N/A |
| Push | APNs | FCM | N/A |
| Clipboard | UIPasteboard (shows indicator) | ClipboardManager (silent) | sensor topic |
| Health data | HealthKit | Health Connect | sensor topic |
