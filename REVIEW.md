# REVIEW.md

Guidance for automated and human reviewers of Buzzdome. Read by Claude Code
Review, the `/review` skill and the reviewer subagents. The invariants are
in `docs/architecture.md`; the conventions in `AGENTS.md`.

## What to flag (in priority order)

1. **Live-show safety:** a state change that reaches a window before the
   journal write; a show that cannot resume after a crash; a panic, unwrap
   or blocking call on the show thread or the windows' thread; a lock held
   while the disk works; a path where missing hardware stops the show.
2. **Rules and replay:** points, correctness or step logic outside
   `quiz-core`; `Game::controls` out of step with `Game::apply`; a change in
   how an old journal replays without a `RULES_VERSION` bump and a gate; a
   changed shipped line in `crates/quiz-core/tests/journal.rs`.
3. **Contracts:** a quiz format change without a default or a
   `SCHEMA_VERSION` bump, or without `docs/quiz-format.md`; a Tauri command
   missing from `build.rs`, `generate_handler!` or the capabilities, or
   given to the projector without need; hand-edited `app/src/bindings`.
4. **Hardware order:** fastest-finger order taken from anything but the
   buzzers' timers or the receiver's order.
5. **Security:** zip-slip or paths escaping the quiz or MCP roots, shell
   arguments to child processes, a loosened CSP, unsigned or unchecked
   downloads, secrets in code or logs, broad workflow permissions.
6. **Tests:** changed behaviour without a test that fails on the old code;
   tests weakened, skipped, `#[ignore]`d or deleted; wall-clock reads or
   unseeded randomness in tests.
7. **UI conventions:** English sentences from Rust instead of `AppError` /
   `Problem` codes; a visible text missing its Czech or English wording;
   typed glyphs instead of `icons.tsx`; colours a theme should decide
   written in stage CSS; a window that scrolls as a whole.
8. **Clean room and names:** text, art or sounds from the reference app or
   a vendor SDK; a trademarked show name in the product.

## What NOT to flag

- What `cargo clippy -D warnings` and ESLint already enforce.
- Style preferences no tool enforces.
- Speculative issues without a concrete failing input.
- Hardware behaviour nobody can check without the receiver: ask for a
  section in `docs/hardware-test.md` instead.

## Severity

- **critical:** a show can fail or lose state mid-event, data loss, a
  security hole, a crash on the main path. Blocks merge.
- **major:** wrong behaviour on a realistic input (wrong points, wrong
  order, a dead button). Fix before merge.
- **minor:** an edge case or maintainability issue. Can be a follow-up.
- **nit:** optional. Report at most three.

Every finding includes `file:line`, a concrete failure scenario (input ->
wrong output) and a suggested fix.
