# Research: how leading teams use AI agents for software development

Snapshot as of 2026-11-07. This is the evidence behind the design in
[`agent-workflow.md`](agent-workflow.md). Each claim links to its source; items marked
*[opinion]* are design judgments of the dev_template workflow, not published results.

## 1. Anthropic (Claude Code)

**Subagents with isolated context and restricted tools.** Claude Code subagents are
Markdown files with YAML frontmatter in `.claude/agents/`; each "runs in its own context
window with a custom system prompt, specific tool access, and independent permissions".
The docs recommend restricting tools to what the job needs (a read-only reviewer gets
`Read, Grep, Glob`), routing cheap work to cheaper models, and checking project subagents
into version control.
Source: Claude Code docs, Subagents - https://code.claude.com/docs/en/sub-agents

**Writer/reviewer separation.** The best-practices guide recommends running a separate
session to review code because a fresh context is not biased toward code it just wrote,
and applying the same split to tests (one session writes tests, another writes code to
pass them). It also stresses giving the agent a check it can run and asking for evidence
rather than asserted success.
Source: Claude Code docs, Best practices - https://code.claude.com/docs/en/best-practices

**Find → Verify → Rank code review.** Anthropic's managed Code Review runs several agents
in parallel over the diff, then a verification step filters false positives, then findings
are deduplicated and ranked by severity. Reviews never approve or block the PR; the merge
decision stays with humans. Review focus is tuned with `REVIEW.md` and `CLAUDE.md`.
Source: Claude Code docs, Code Review - https://code.claude.com/docs/en/code-review

**Orchestrator-worker multi-agent systems.** In Anthropic's research system, a lead agent
plans and spawns parallel subagents; Opus 4 lead + Sonnet 4 subagents outperformed
single-agent Opus 4 by 90.2% on their internal research eval. The cost: multi-agent
systems used about 15x more tokens than chats, so they pay off only for high-value,
parallelizable tasks. Subagents need a clear objective, output format, tool guidance and
task boundaries to avoid duplicated work.
Source: Anthropic Engineering, "How we built our multi-agent research system" -
https://www.anthropic.com/engineering/multi-agent-research-system

**Workflow patterns.** Prompt chaining, routing, parallelization (sectioning/voting),
orchestrator-workers, and evaluator-optimizer (generator + separate critic in a loop).
Core advice: start simple and add agentic complexity only when it measurably helps.
Source: Anthropic Engineering, "Building effective agents" -
https://www.anthropic.com/engineering/building-effective-agents

**Long-running work across sessions.** An initializer session creates `init.sh`, a
progress file and an initial commit; later sessions work on one feature at a time, read
the progress log and git history at start, and commit at the end. Features are tracked in
a JSON list where agents may only flip `passes`, because models were "less likely to
inappropriately change or overwrite JSON files" than Markdown. Browser-based end-to-end
testing "as a human user would" greatly improved results versus unit tests or curl only.
Source: Anthropic Engineering, "Effective harnesses for long-running agents" -
https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents

**Memory and instructions.** `CLAUDE.md` is loaded every session; target under 200 lines,
because longer files reduce adherence. `@path` imports, path-scoped `.claude/rules/` and
skills keep context small. Claude Code reads `AGENTS.md` when no `CLAUDE.md` exists, or via
`@AGENTS.md` import.
Source: Claude Code docs, Memory - https://code.claude.com/docs/en/memory

**Skills.** `.claude/skills/<name>/SKILL.md`; only the description is always in context,
the body loads on use (progressive disclosure); keep `SKILL.md` under 500 lines;
`disable-model-invocation: true` for side-effecting workflows.
Source: Claude Code docs, Skills - https://code.claude.com/docs/en/skills

**Hooks as deterministic guardrails.** "Unlike CLAUDE.md instructions which are advisory,
hooks are deterministic". `PreToolUse` hooks exiting with code 2 block the tool call.
Sources: https://code.claude.com/docs/en/best-practices, https://code.claude.com/docs/en/hooks

**CI integration.** `anthropics/claude-code-action@v1` responds to `@claude` mentions or
runs a prompt/skill on events such as `pull_request`.
Source: Claude Code docs, GitHub Actions - https://code.claude.com/docs/en/github-actions

## 2. OpenAI (Codex)

**AGENTS.md.** "A simple, open format for guiding coding agents" - a README for agents.
Source: https://github.com/openai/agents.md (agents.md project)

Codex concatenates `AGENTS.md` files from the repo root down to the working directory;
closer files override earlier guidance; `AGENTS.override.md` takes priority in a
directory; default combined size limit 32 KiB.
Source: OpenAI Codex docs, AGENTS.md - https://developers.openai.com/codex/guides/agents-md

**Custom subagents** are TOML files in `.codex/agents/` with required `name`,
`description`, `developer_instructions`; sandbox and approval settings are inherited from
the parent session, not set per agent.
Source: OpenAI Codex docs, Subagents - https://developers.openai.com/codex/subagents

**Skills** are discovered from `.agents/skills` directories between the working directory
and repo root; `SKILL.md` requires `name` and `description`.
Source: OpenAI Codex docs, Skills - https://developers.openai.com/codex/skills

## 3. GitHub (Copilot)

**Custom agents** are `.agent.md` files in `.github/agents/` with `name`, `description`
and optional `tools`.
Source: GitHub Docs, Create custom agents for CLI -
https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/create-custom-agents-for-cli

**Instruction files.** Copilot coding agent supports `AGENTS.md` (including nested
files), `.github/copilot-instructions.md`, `.github/instructions/**.instructions.md`,
and also reads `CLAUDE.md` and `GEMINI.md`.
Source: GitHub Changelog 2025-08-28 -
https://github.blog/changelog/2025-08-28-copilot-coding-agent-now-supports-agents-md-custom-instructions

## 4. Testing and UX tooling

**Playwright Test Agents:** planner (explores the app and writes a Markdown test plan),
generator (turns the plan into Playwright tests, verifying selectors live), healer (runs
the suite and repairs failing tests, or skips a test if it believes the feature is broken).
Source: Playwright docs, Test agents - https://playwright.dev/docs/test-agents

**Accessibility baseline:** WCAG 2.2 - https://www.w3.org/TR/WCAG22/
**Security baseline:** OWASP Top 10 - https://owasp.org/Top10/

## 5. What Buzzdome takes from it

| Practice | Where in this repo |
| --- | --- |
| Specialist subagents, least-privilege tools | `.claude/agents/` |
| Writer ≠ reviewer, test writer ≠ implementer | `test-engineer` / `implementer` / reviewers |
| Parallel find → independent verify → rank | `/review` skill, `verifier` agent |
| Evaluator-optimizer loop with a budget | `/feature` step 6 (max 3 rounds) |
| Evidence over claims | `AGENTS.md` (Agent workflow, 3), PR template |
| Real-browser UX/a11y review | `ux-ui-reviewer`, `.mcp.json` (Playwright MCP) |
| Adversarial testing with reproductions | `bug-hunter`, `/bug-hunt` |
| JSON feature list + progress log + git | `memory/features.json`, `memory/progress.md` |
| Short always-loaded instructions, details on demand | `AGENTS.md`, `.claude/rules/`, skills |
| Deterministic guardrails | `.claude/hooks/`, `.claude/settings.json`, CI |
| One source, many tools | `AGENTS.md`, `scripts/sync_agents.py` |
| Second, independent review in CI; humans merge | `.github/workflows/claude-review.yml` (off until enabled) |

*[opinion]* The `bug-hunter` + `verifier` pair is the dev_template workflow's generalisation of
Code Review's verification step to dynamic testing: the hunter must produce a reproduction,
the verifier must re-run it. No source above publishes that exact pairing.
