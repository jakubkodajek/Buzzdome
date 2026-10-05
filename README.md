# Quiz show (working title)

A desktop quiz and game-show app for live quiz nights at non-profit events:
questions on the projector, players answering with wireless buzzers, the
quizmaster in control from the laptop. Windows first, built with Tauri
(Rust core, React UI).

Inspired by Windows-only game-show software. This is an independent project,
not affiliated with or endorsed by QuizXpress or its makers.

## Status

First playable version: demo quiz, everyone-answers and fastest-finger
questions, scores, leaderboard, final scores, crash recovery, keyboard and
on-screen buzzers. Hardware buzzer support is being researched
([`replica/hardware.md`](replica/hardware.md)). Plan and progress:
[`ROADMAP.md`](ROADMAP.md).

## Development

Needs Rust (stable) and Node.js.

```sh
cd app
npm install
npm run tauri dev      # run the app
cd .. && cargo test --workspace
```

Project instructions for Claude Code are in `CLAUDE.md`. What the app should do
is mapped in [`replica/recon.md`](replica/recon.md) and prioritised in
[`replica/features.csv`](replica/features.csv).
