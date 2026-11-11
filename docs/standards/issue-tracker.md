# Issue Tracking & Triage Standards

Issues, specs and tickets live in **GitHub Issues** for `minhkhang1008/ctdozlite`. Use the `gh` CLI for every operation.

> Source of truth for ticket state. Local markdown notes under `re-notes/` are evidence, never tickets.

## 1. CLI Command Conventions

| Operation | Command | Notes |
| :--- | :--- | :--- |
| Create issue | `gh issue create --title "..." --body "..."` | Use a heredoc for structured bodies. |
| View issue | `gh issue view <number> --comments` | Description, labels and discussion. |
| List issues | `gh issue list --state open --label "<label>"` | Filter by lifecycle, type or area. |
| Comment | `gh issue comment <number> --body "..."` | Post updates, link PRs, attach evidence. |
| Apply labels | `gh issue edit <number> --add-label "<label>"` | Update type/area/priority. |
| Close issue | `gh issue close <number> --comment "..."` | Always close with a resolution summary. |

## 2. Label Taxonomy

### A. Lifecycle (exceptions only)

State lives in the issue itself (`open`/`closed`), PR links (`Fixes #<id>`) and GitHub Projects. Do not maintain `in-progress` / `needs-review` labels.

| Label | Meaning |
| :--- | :--- |
| `status:blocked` | Waiting on an external dependency, credentials, upstream protocol evidence or another issue. |
| `wontfix` | Closed as out of scope, superseded or rejected. |

### B. Type (Conventional Commits alignment)

`type:feat`, `type:fix`, `type:refactor`, `type:perf`, `type:test`, `type:docs`, `type:chore`, `type:ci`, `type:style`, `type:task`.

### C. Area (domain scopes)

| Label | Scope |
| :--- | :--- |
| `area:auth` | QR/password login, `session.json`, `zpw_sek` / `zpw_enk`. |
| `area:net` | HTTP endpoints, WebSocket transport, `ParamCipher v2`, `signkey`. |
| `area:chat` | Threads, messages, reactions, recall, forwarding. |
| `area:media` | Attachments, images, stickers, voice payloads. |
| `area:call` | VoIP signalling, audio/video capture. |
| `area:ui` | egui GUI, ratatui TUI, theme, fonts. |
| `area:store` | SQLite schema, queries, local history. |
| `area:ci` | GitHub Actions, packaging, release artifacts. |
| `area:harness` | `AGENTS.md`, `docs/standards/`, skills, agent tooling. |

### D. Priority

| Label | Meaning |
| :--- | :--- |
| `priority:p0` | Critical: data loss, credential exposure, complete outage. |
| `priority:p1` | Blocks the next release milestone. |
| `priority:p2` | Normal (default). |

## 3. Triage Label Vocabulary

Canonical vocabulary consumed by `/triage`. Do not introduce synonyms.

| Role | Repo representation |
| :--- | :--- |
| `needs-triage` | Default: open issue with no `type:*` / `area:*` label. |
| `needs-info` | `status:blocked` + a comment naming the missing evidence. |
| `ready-for-agent` | `type:*` + `area:*` set, acceptance criteria present, no open blocker. |
| `ready-for-human` | `priority:p0` / `priority:p1`, or requires protocol evidence / physical device testing. |
| `wontfix` | `wontfix` label + closing comment. |

## 4. Native Sub-Issues Protocol

1. Create the child:

   ```bash
   gh issue create --title "task(<area>): <summary>" --body "..." --label "type:task,area:<area>"
   ```

2. Link it to the parent:

   ```bash
   CHILD_ID=$(gh api repos/minhkhang1008/ctdozlite/issues/<child_number> --jq .id)
   gh api --method POST repos/minhkhang1008/ctdozlite/issues/<parent_number>/sub_issues -F sub_issue_id=$CHILD_ID
   ```

3. Query children:

   ```bash
   gh api repos/minhkhang1008/ctdozlite/issues/<parent_number>/sub_issues \
     --jq '.[] | "#\(.number): \(.title)"'
   ```

## 5. Wayfinding

- **Map ticket**: one issue labeled `wayfinder:map`, holding decisions and the fog tracker.
- **Child tickets**: labeled `wayfinder:<research|prototype|grilling|task>` and linked as native sub-issues.
- **Blocking**: use native issue dependencies; a ticket is unblocked when every blocker is closed.

## 6. Ticket Quality Bar

A ticket is actionable only when it carries:

- [ ] One-sentence outcome (observable behaviour, not file inventory).
- [ ] Reproduction or acceptance checklist.
- [ ] Affected area label.
- [ ] Evidence for protocol changes (capture path, `re-notes/` reference, server `error_code`).
- [ ] Explicit non-goals when scope is easy to over-read.

## 7. Security Rule

Never paste `session.json`, `zpw_sek`, `zpw_enk`, cookies, IMEI or raw captures containing them into an issue. Redact to `zpw_sek=<redacted>` and attach only sanitized excerpts.
