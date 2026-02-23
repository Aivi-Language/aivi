# Mailfox — Clean-Slate Architecture Plan

GNOME/Wayland-first, GTK4 + libadwaita mail client with an AIVI-driven intelligence daemon.

## 1) Product contract and non-negotiables

Mailfox must optimize for meaningful work, not raw inbox volume.

Hard invariants:

- Multi-account IMAP with OAuth2-capable providers.
- Email records are immutable once ingested.
- AI extraction is idempotent and never auto-rerun for already-extracted emails.
- Account removal is soft-delete; re-adding an account must reuse prior immutable emails and extractions.
- Only Danger Zone full wipe can remove immutable historical data and extraction cache.
- SQLite is the only backend.

---

## 2) System architecture

## 2.1 Three-part runtime

1. **GTK app (Mailfox UI process)**
   - Presents views, captures user intents, reads projections.
   - Writes commands to local durable queue.
2. **mailfoxd (AIVI daemon process)**
   - Syncs IMAP, refreshes OAuth, performs extraction orchestration, executes plugins, sends notifications.
   - Owns write-side consistency and scheduling.
3. **AI restpoint (external HTTP)**
   - One email request -> one JSON extraction payload.
   - Versioned schema/model metadata.

## 2.2 Bounded contexts

- Account Identity & Auth
- Mail Ingestion & Normalization
- Extraction & Plugin Projection
- Threading & Conversation Model
- Actions/Outbox
- Notifications
- Settings & Safety (Danger Zone)

---

## 3) Data architecture (SQLite, immutable-first)

## 3.1 Identity and connectivity

- `accounts`
  - account metadata, provider, sync_since, soft-delete markers.
- `oauth_tokens`
  - keyring handle + metadata (not plaintext secrets).
- `mailboxes`
  - UIDVALIDITY, sync cursors, role mapping.

## 3.2 Immutable mail substrate

- `emails`
  - immutable message body/headers + stable identifiers.
- `email_account_links`
  - account-mailbox-specific mutable flags (`seen`, `archived`, etc.).
- `threads` + `thread_members`
  - stable conversation grouping.
- `attachments`
  - metadata + content-addressed storage path/hash.

## 3.3 Extraction and projection

- `extractions`
  - immutable JSON payload + `(schema_version, prompt_version, model_id)` keys.
  - unique key includes `email_id` + versioning tuple.
- `derived_todos`
- `derived_events`
- `derived_bills`
- `derived_subscriptions`
- `derived_job_apps`
- `derived_status_updates`
- `conversation_utterances`
- `render_docs_markdown`

All derived tables reference `email_id`, `extraction_id`, `plugin_version`.

## 3.4 Operations and durability

- `commands` (UI -> daemon)
- `jobs` (daemon durable queue with dedupe key, attempts, run_after)
- `scheduler_runs` / `scheduler_leases` / `scheduler_heartbeats`
- `tombstones` (soft-delete/audit/full wipe traces)
- `notifications_log`

## 3.5 Search

Use FTS5 virtual tables for:

- subject/from/snippet
- markdown render docs
- extracted entities and utterances

---

## 4) Sync and ingestion flow

Per account sync loop:

1. Acquire account lease.
2. Ensure valid OAuth token (refresh if needed).
3. Sync mailbox metadata and UID cursors.
4. Fetch new/changed message envelopes, then bodies/attachments.
5. Normalize and persist immutable email/attachment rows.
6. Enqueue extraction jobs for newly ingested emails.
7. Emit per-account progress snapshots for status bars.

Progress model (for detailed account status bars):

- `phase`: `connect | list_mailboxes | fetch_headers | fetch_bodies | extract | plugins | idle`
- counters: `processed`, `total`, `errors`, `rate_per_min`
- sample display: `AI processing 21/293`.

---

## 5) AI extraction contract and idempotency

## 5.1 Restpoint payload

Single normalized request per email, optionally with lightweight thread context and attachment text references.

## 5.2 Response schema (canonical)

- `classification`: categories, urgency, sentiment, confidence
- `entities`: people, orgs, dates, money, invoice/subscription IDs
- `actionables`: todos, schedules, bills, job application updates
- `thread`: utterances (speaker, text, timestamp)
- `cleaned_content`: markdown summary + key excerpts

## 5.3 Never-rerun enforcement

Before extraction:

- lookup by `(email_id, schema_version, prompt_version, model_id)`.
- if success exists: skip always.
- if failed exists: retry only explicit policy/manual action.

Account re-add:

- link account back via `email_account_links`; do not create new extraction rows.

---

## 6) Local plugin pipeline

Execution order:

1. `RemoteExtractionPlugin` (REST call + persist extraction)
2. `TodosProjectionPlugin`
3. `ScheduleProjectionPlugin`
4. `BillsProjectionPlugin`
5. `SubscriptionsProjectionPlugin`
6. `JobAppProjectionPlugin`
7. `StatusUpdateProjectionPlugin`
8. `ConversationProjectionPlugin`
9. `MarkdownRenderPlugin`

Plugin rules:

- deterministic upserts
- idempotent by `(email_id, plugin_version, logical_key)`
- no mutation of immutable source email row

Attachment strategy:

- first pass: index metadata and hash
- optional secondary extraction job (PDF/text) with separate caching and budget controls

---

## 7) Thread reconstruction and chat-style rendering

Thread reconstruction precedence:

1. RFC headers (`Message-ID`, `In-Reply-To`, `References`)
2. provider thread hints (if available)
3. fallback heuristics (subject normalization + participant overlap + time proximity)

Drawer chat rendering:

- show extracted utterances in messenger bubbles
- collapse quoted signatures/chrome by default
- explicit “Show original email” toggle

---

## 8) GTK4 + libadwaita UI blueprint

## 8.1 Main shell

- Left icon-only sidebar:
  - Inbox/Mail, Todos, Schedule, Bills, Subscriptions, Search, Settings
  - bottom: readiness indicator + settings shortcut
- Top bar:
  - account filter (`All accounts` + specific)
  - global quick search with hint `Press Ctrl+K`
- Main canvas:
  - grouped inbox columns by time buckets (Today/Yesterday/Last week etc.)
  - view-specific boards (Todo kanban lanes etc.)

## 8.2 Selection and actions

- Selecting one/many cards opens floating bottom action bar:
  - delete/trash, reply/forward, move, privacy/hide, print, bulk actions
- During drag start, show compact drop-target action strip.

## 8.3 Right drawer

- resizable side drawer with drag handle
- header: subject + sender/provider + timestamp
- body: cleaned markdown render doc
- thread/chat subview
- bottom quick reply composer:
  - formatting mini-toolbar (bold/italic/strike/link/attach)
  - Send button

## 8.4 Settings

- top tabs: Accounts / Danger Zone
- save changes + close actions
- Accounts page:
  - left account list cards + reorder handles + add/remove
  - right segmented tabs: General / Incoming / Outgoing
  - General fields: account name, readonly email, sync-since date picker
- LLM provider panel:
  - API key, model, base URL (default OpenAI URL), organization optional
- additional app panels:
  - General
  - Quick View

---

## 9) Notifications and tray behavior

- Daemon emits new-mail notifications with account-aware context.
- Notification click opens target email drawer in UI.
- Quick actions: Reply / Archive / Mark done. Open a small window next to tray icon.
- Tray/status integration:
  Ship the extension bundled with your Rust app.
  On first run:
    Extract it into ~/.local/share/gnome-shell/extensions/<uuid>/
    Run gnome-extensions enable <uuid>
    Inform the user that a session restart may be required.

---

## 10) AIVI usage model (where it fits best)

Primary use in daemon/orchestration:

- state machines for sync/extraction/action lifecycles
- generators for staged pipelines and batching
- scheduler + durable queue orchestration
- external sources for IMAP/REST/env/file
- resources/effects for connection and failure boundaries

Proposed state machines:

- `AccountSyncMachine`
- `ExtractionMachine`
- `PluginFanoutMachine`
- `CommandExecutionMachine`

---

## 11) App features vs stdlib boundary (important)

Mailfox-specific functionality should stay in Mailfox app modules, not AIVI stdlib:

- Todo/Bills/JobApp projection semantics
- Kanban lane policies and UI behaviors
- Drawer composition and quick-reply UX
- account-scoped progress bar semantics
- danger-zone wipe policy and product-level governance

AIVI stdlib/runtime should provide generic primitives only (DB, scheduling, concurrency, secrets, protocols, validation).

---

## 12) Delivery roadmap

1. Foundation: schema, migrations, account setup, daemon skeleton
2. Sync MVP: IMAP ingest + immutable store + thread index
3. Extraction MVP: one-restpoint JSON + immutable extraction cache
4. Projection MVP: todos/events/bills + markdown drawer rendering
5. UX MVP: sidebar+search+account filter+drawer+floating actions
6. Notifications + quick reply + outbox
7. Hardening: performance, keyring, observability, full danger-zone workflows
