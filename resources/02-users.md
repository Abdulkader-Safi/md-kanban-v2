# Users and usage

Status: signed off 2026-10-11

## Personas

### Primary: "Safi," the lead engineer who runs agents all day
- **Background:** Senior and lead software engineer at dsrpt (Kuwait and Australia), plus personal open-source projects. Runs Claude Code on several repos at once, often two projects side by side (today: MD-Kanban itself and clinicos, 169 cards between them).
- **What drives him:** wants to see at a glance what each agent did, what is in review, and what is stuck, without reading chat logs. Wants tools he owns, that run offline, and that cost nothing to leave open. Likes Vim keys, keyboard-first flows and dark UIs.
- **Current workaround:** the Electron MD Kanban. It works, but it is a 100+ MB Chromium app for a list of text files, and its file handling runs through two runtimes.
- **Tech comfort:** very high. Builds from source, edits YAML by hand, lives in NeoVim and Zed.
- **Relationship to AI agents:** delegates real work to them daily and treats the board as the shared to-do list between him and the agent. Wants the agent to need nothing but file access.
- **Devices and environment:** MacBook (Apple Silicon), large external display, dark theme. Projects live in local git repos. English UI.

### Secondary: "Dana," the developer who found it on GitHub
- **Background:** a software engineer at a startup or freelancing, using Claude Code, Codex or Cursor agents every day. Finds mdk through Safi's posts, a GitHub search for "kanban markdown agents", or a Hacker News thread. Could be on macOS, Windows or Linux.
- **What drives them:** the same blindness to what the agent is doing, plus fatigue with heavy Electron apps. Will try a native app because it is native.
- **Current workaround:** a `TODO.md` the agent edits, GitHub Issues, Linear, or Backlog.md in a terminal.
- **Tech comfort:** high. Will download a release, accept an unsigned-app warning if the README explains it, and point an agent at `AGENTS.md`. Will not compile Rust just to try it.
- **Relationship to AI agents:** heavy, pragmatic user. Wants the agent instructions to be one file they can paste.
- **Devices and environment:** any of the three desktop OSes, often a laptop screen (1440 px wide or less), light or dark theme.
- **Why they matter:** stars, issues and word of mouth are what make mdk a portfolio piece. **Why not the beachhead:** v1 is built for Safi's daily use first. Dana gets a good README, release builds and a sample project, not features of their own.

### Tertiary: the AI agent
- **Who:** Claude Code, Codex, or any agent with file access to the project folder.
- **What it needs:** a stable, documented file format (`AGENTS.md`); its edits picked up without any signal to the app; its extra frontmatter keys never deleted; no lock files, no database, no hidden state that disagrees with the files; and the app never overwriting a file the agent just changed.
- **What it does:** reads cards, creates card files, edits bodies and checkboxes, changes `status` to move cards, deletes files only when asked.
- **Contract:** the file format and `AGENTS.md` from the Electron repo, unchanged, so the same folder works in both apps.

### Not the target
- Teams that need shared real-time boards, permissions, comments or notifications. That is Linear or GitHub Projects.
- People who want the app to run or orchestrate agents. That is Vibe Kanban.
- Non-technical users. mdk assumes you know what a folder, markdown and frontmatter are.
- Mobile users. Desktop only.

## Roles and permissions
Single-user desktop app, no accounts and no roles. The only "permission" is the operating system's file access to the folders the user connects. The AI agent is not a role inside the app: it acts on the files directly.

## Jobs to be done
- When **Safi starts an agent on a feature**, he wants to **see its cards move across the board as it works**, so he can **step in only when something lands in Review or gets stuck**.
- When **Safi plans work**, he wants to **create cards fast with one line (`Fix login p1 #backend @safi due:friday`)**, so he can **brief the agent without filling in forms**.
- When **a card needs detail**, Safi wants to **write markdown with a toolbar, Vim keys, code blocks, tables, maths and diagrams, and see it rendered**, so **the card is the full spec the agent works from**.
- When **an agent finishes**, Safi wants to **read its notes, tick subtasks and drag the card to Done or back**, so **the board stays the truth**.
- When **a card is ready for an agent**, Safi wants to **copy a ready-made prompt that points the agent at the file**, so **handing off takes one click**.
- When **Done gets long**, Safi wants to **archive all done cards into `archive/`**, so **the board stays readable and history stays in git**.
- When **Dana finds mdk**, they want to **download it, connect a folder and see their first card in under two minutes**, so they **decide to keep it**.
- When **the agent works**, it wants to **edit a card file and have that change show, untouched, with its own keys kept**, so **it never fights the app**.

## Primary user journey
1. **Discovery:** Safi already uses MD Kanban. Dana sees a post or a GitHub result: "a native kanban your AI agents update by editing markdown files, about 10x smaller than Electron".
2. **Setup:** download the release for their OS, open it (one Gatekeeper or SmartScreen warning, explained in the README), click Connect folder, pick a project folder. Under two minutes, no account.
3. **First use:** the board fills with every `.md` file that has task frontmatter, grouped into five columns, with work folders listed in the sidebar. An empty folder shows the empty state with "Create first card".
4. **Aha moment:** they tell the agent "read AGENTS.md and work through the To Do cards", then watch a card move to In Progress on its own, without touching the app.
5. **Optional power-up:** ⌘K palette, quick-add syntax, saved views, Vim keys, keyboard card moving, list view.
6. **Finish:** a card reaches Done. Done cards get archived into `archive/` and committed with the code.
7. **Ongoing use:** mdk stays open all day next to the editor. It is the first thing Safi checks when he comes back to a project.

## Core loop
**The repeated action:** write or quick-add a card -> hand it to the agent (Copy AI prompt or "work the To Do column") -> watch it move -> open, review, tick subtasks -> drag to Done -> archive.

**What pulls users back:**
- The board updates itself from agent edits, so it is always current without effort.
- It is cheap enough to never close.
- The cards are plain files in git, so nothing is locked in and history is free.
- Filters and saved views per project ("my overdue", "backend high") make a 100-card project scannable.

## Language, accessibility and locale
- **Language:** English UI only. No right-to-left support: Arabic text in cards is not a v1 requirement.
- **Dates:** due dates are ISO `YYYY-MM-DD` in files. On screen, relative labels ("Today", "Tomorrow", "3d", "2d overdue") plus the ISO date, as in the Electron app. Times shown in the user's local time zone.
- **Accessibility:** keyboard and contrast, no screen readers in v1:
  - Every action reachable by keyboard, including moving cards between and within columns (Space to pick up, arrows to move, Space to drop, Esc to cancel).
  - Visible focus on every focusable element.
  - Text contrast at least 4.5:1 and UI contrast at least 3:1, in both light and dark themes.
  - Respect the OS "reduce motion" setting.
  - Screen readers (VoiceOver, NVDA, Orca) are out of scope for v1. Controls still get names where gpui-kit sets them for free, so adding screen-reader support later doesn't mean rewriting.
- **Fonts:** Geist Sans for UI text and a system monospace for paths, work folders and the editor, matching the Electron app. Fonts are bundled.
- **Screen sizes:** from a 1280 x 800 laptop window up to a 5K display. No mobile layout.

---

### Open questions
None.

### Assumptions confirmed
1. **Secondary persona : RESOLVED.** Developers running AI agents who find mdk on GitHub or through Safi's posts.
2. **The agent as a user : RESOLVED.** Yes. Its needs (stable format, kept keys, no lock files, no overwriting its edits) become explicit requirements in stage 3.
3. **Right-to-left : RESOLVED.** Not needed. English only, no RTL.
4. **Accessibility : RESOLVED.** Keyboard access and contrast (4.5:1 text, 3:1 UI) in both themes, plus reduce motion. No screen-reader work in v1: mdk is a developer tool Safi uses himself, and screen-reader support in GPUI is new and would need testing on three platforms.
