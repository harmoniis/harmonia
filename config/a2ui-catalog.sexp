;;; a2ui-catalog.sexp — A2UI Component Registry v1.0
;;; Canonical definitions of all available template components.
;;; The conductor injects these definitions into signals from A2UI-capable frontends,
;;; so the LLM knows exactly which components exist and how to parameterize them.
;;;
;;; Each component: (:name <string> :description <string> :fields <field-list>)
;;; Each field:     (:name <string> :type <string> :required <t|nil> :default <value|nil>)

(:version "1.0"
 :render-topic "harmonia/{agent_id}/cmd/{device_id}/render"
 :envelope (:id "string — unique widget instance ID"
            :component "string — component name from catalog"
            :data "object — component-specific fields"
            :position "timeline | overlay | replace"
            :priority "low | normal | high | urgent"
            :replace_id "string? — replaces widget with this ID"
            :expires_at "ISO-8601? — auto-remove after this time")
 :components
 ((:name "TextBubble"
   :description "Agent text response with optional audio playback"
   :fields ((:name "text" :type "string" :required t)
            (:name "audio_url" :type "string" :required nil)
            (:name "markdown" :type "bool" :required nil :default nil)))

  (:name "VoiceWaveform"
   :description "Audio playback with waveform visualization"
   :fields ((:name "audio_url" :type "string" :required t)
            (:name "duration_ms" :type "int" :required t)
            (:name "waveform_data" :type "float[]" :required nil)))

  (:name "MapWithList"
   :description "Map view with POI markers and scrollable list"
   :fields ((:name "center_lat" :type "float" :required t)
            (:name "center_lon" :type "float" :required t)
            (:name "zoom" :type "int" :required nil :default 14)
            (:name "points" :type "object[]" :required t)
            (:name "list_action_label" :type "string" :required nil)))

  (:name "MediaViewer"
   :description "Display an image or video"
   :fields ((:name "url" :type "string" :required t)
            (:name "media_type" :type "image|video" :required t)
            (:name "caption" :type "string" :required nil)
            (:name "aspect_ratio" :type "float" :required nil)))

  (:name "ListTable"
   :description "Data table with optional sorting and row actions"
   :fields ((:name "headers" :type "string[]" :required t)
            (:name "rows" :type "string[][]" :required t)
            (:name "sortable" :type "bool" :required nil :default nil)
            (:name "row_actions" :type "object[]" :required nil)))

  (:name "ChoiceChips"
   :description "Quick-reply buttons, user taps one and MQTT event fires"
   :fields ((:name "choices" :type "object[]" :required t)
            (:name "callback_topic" :type "string" :required t)
            (:name "multi_select" :type "bool" :required nil :default nil)))

  (:name "DeepLink"
   :description "Card that opens a URL scheme when tapped"
   :fields ((:name "label" :type "string" :required t)
            (:name "url" :type "string" :required t)
            (:name "icon" :type "string" :required nil)
            (:name "description" :type "string" :required nil)))

  (:name "FormInput"
   :description "Dynamic form with fields and validation"
   :fields ((:name "fields" :type "object[]" :required t)
            (:name "submit_label" :type "string" :required nil :default "Submit")
            (:name "submit_topic" :type "string" :required t)))

  (:name "CodeBlock"
   :description "Syntax-highlighted code display"
   :fields ((:name "language" :type "string" :required nil)
            (:name "code" :type "string" :required t)
            (:name "line_numbers" :type "bool" :required nil :default t)
            (:name "copyable" :type "bool" :required nil :default t)))

  (:name "ProgressTracker"
   :description "Multi-step progress indicator"
   :fields ((:name "title" :type "string" :required nil)
            (:name "steps" :type "string[]" :required t)
            (:name "current_step" :type "int" :required t)))

  (:name "Calendar"
   :description "Calendar view with events"
   :fields ((:name "view_mode" :type "day|week|month" :required nil :default "week")
            (:name "events" :type "object[]" :required t)
            (:name "action_topic" :type "string" :required nil)))

  (:name "Timer"
   :description "Countdown or countup timer"
   :fields ((:name "duration_ms" :type "int" :required t)
            (:name "label" :type "string" :required nil)
            (:name "auto_start" :type "bool" :required nil :default nil)
            (:name "callback_topic" :type "string" :required nil)))

  (:name "PermissionCard"
   :description "Explains why a permission is needed with a grant button"
   :fields ((:name "permission_type" :type "string" :required t)
            (:name "reason" :type "string" :required t)
            (:name "grant_topic" :type "string" :required t)))

  (:name "WalletCard"
   :description "Webcash balance and recent transactions"
   :fields ((:name "balance" :type "string" :required t)
            (:name "currency" :type "string" :required nil :default "webcash")
            (:name "transactions" :type "object[]" :required nil)
            (:name "action_topic" :type "string" :required nil)))

  (:name "ContactCard"
   :description "Contact info with action buttons"
   :fields ((:name "name" :type "string" :required t)
            (:name "phone" :type "string" :required nil)
            (:name "email" :type "string" :required nil)
            (:name "avatar_url" :type "string" :required nil)
            (:name "actions" :type "object[]" :required nil)))

  (:name "WeatherCard"
   :description "Weather display with current and forecast"
   :fields ((:name "location" :type "string" :required t)
            (:name "current" :type "object" :required t)
            (:name "forecast" :type "object[]" :required nil)))

  (:name "AudioPlayer"
   :description "Full audio player with scrubbing"
   :fields ((:name "url" :type "string" :required t)
            (:name "title" :type "string" :required nil)
            (:name "waveform_data" :type "float[]" :required nil)))

  (:name "ImageGallery"
   :description "Swipeable image gallery"
   :fields ((:name "images" :type "object[]" :required t)))

  (:name "Notification"
   :description "In-app notification banner"
   :fields ((:name "title" :type "string" :required t)
            (:name "body" :type "string" :required nil)
            (:name "priority" :type "low|normal|high|urgent" :required nil :default "normal")
            (:name "actions" :type "object[]" :required nil)
            (:name "auto_dismiss_ms" :type "int" :required nil)))

  (:name "Separator"
   :description "Visual divider with optional label"
   :fields ((:name "style" :type "line|space|dot" :required nil :default "line")
            (:name "label" :type "string" :required nil)))

  (:name "Composite"
   :description "Container that nests other components for complex layouts"
   :fields ((:name "layout" :type "vertical|horizontal|grid" :required nil :default "vertical")
            (:name "spacing" :type "int" :required nil :default 8)
            (:name "children" :type "component[]" :required t)))))
