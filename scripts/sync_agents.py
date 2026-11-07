#!/usr/bin/env python3
"""Generate agent and skill definitions for other tools from `.claude/`.

Single source of truth:
    .claude/agents/*.md        Claude Code subagents
    .claude/skills/*/SKILL.md  Agent skills

Generated (do not edit by hand):
    .github/agents/<name>.agent.md   GitHub Copilot custom agents
    .codex/agents/<name>.toml        OpenAI Codex custom subagents
    .agents/skills/<name>/...        Codex skill discovery path

Usage:
    python3 scripts/sync_agents.py          # write generated files
    python3 scripts/sync_agents.py --check  # exit 1 if generated files are stale (CI)
"""

from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLAUDE_AGENTS = ROOT / ".claude" / "agents"
CLAUDE_SKILLS = ROOT / ".claude" / "skills"
GENERATED_HEADER = "Generated from .claude/{src} by scripts/sync_agents.py - do not edit."

REQUIRED_AGENT_FIELDS = ("name", "description")


@dataclass(frozen=True)
class AgentDef:
    """A parsed Claude Code subagent definition."""

    source: Path
    meta: dict[str, str]
    body: str

    @property
    def name(self) -> str:
        return self.meta["name"]


def parse_frontmatter(text: str, source: Path) -> tuple[dict[str, str], str]:
    """Parse simple `key: value` YAML frontmatter without third-party deps."""
    if not text.startswith("---\n"):
        raise ValueError(f"{source}: missing frontmatter (file must start with '---')")
    try:
        header, body = text[4:].split("\n---\n", 1)
    except ValueError as exc:
        raise ValueError(f"{source}: unterminated frontmatter") from exc
    meta: dict[str, str] = {}
    for line_no, line in enumerate(header.splitlines(), start=2):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.startswith((" ", "\t", "-")):
            # Nested YAML (lists/maps) is allowed but not needed for generation.
            continue
        if ":" not in line:
            raise ValueError(f"{source}:{line_no}: invalid frontmatter line: {line!r}")
        key, value = line.split(":", 1)
        meta[key.strip()] = value.strip().strip('"').strip("'")
    return meta, body.lstrip("\n")


def load_agents() -> list[AgentDef]:
    agents: list[AgentDef] = []
    names: set[str] = set()
    for path in sorted(CLAUDE_AGENTS.glob("*.md")):
        meta, body = parse_frontmatter(path.read_text(encoding="utf-8"), path)
        missing = [f for f in REQUIRED_AGENT_FIELDS if not meta.get(f)]
        if missing:
            raise ValueError(f"{path}: missing required field(s): {', '.join(missing)}")
        if meta["name"] != path.stem:
            raise ValueError(f"{path}: name '{meta['name']}' must match filename '{path.stem}'")
        if meta["name"] in names:
            raise ValueError(f"{path}: duplicate agent name '{meta['name']}'")
        names.add(meta["name"])
        agents.append(AgentDef(path, meta, body))
    return agents


def validate_skills() -> list[Path]:
    skill_dirs: list[Path] = []
    for skill_md in sorted(CLAUDE_SKILLS.glob("*/SKILL.md")):
        meta, _ = parse_frontmatter(skill_md.read_text(encoding="utf-8"), skill_md)
        if not meta.get("description"):
            raise ValueError(f"{skill_md}: missing 'description'")
        if meta.get("name", skill_md.parent.name) != skill_md.parent.name:
            raise ValueError(f"{skill_md}: name must match directory name")
        skill_dirs.append(skill_md.parent)
    return skill_dirs


def toml_string(value: str) -> str:
    """Return a TOML basic string literal."""
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def toml_multiline(value: str) -> str:
    """Return a TOML multi-line literal string (no escaping inside)."""
    if "'''" in value:
        raise ValueError("agent body must not contain ''' (TOML literal delimiter)")
    return f"'''\n{value.rstrip()}\n'''"


def render_copilot(agent: AgentDef) -> str:
    header = GENERATED_HEADER.format(src=f"agents/{agent.source.name}")
    return (
        "---\n"
        f"name: {agent.name}\n"
        f"description: {toml_string(agent.meta['description'])}\n"
        "---\n\n"
        f"<!-- {header} -->\n\n"
        f"{agent.body.rstrip()}\n"
    )


def render_codex(agent: AgentDef) -> str:
    header = GENERATED_HEADER.format(src=f"agents/{agent.source.name}")
    return (
        f"# {header}\n"
        f"name = {toml_string(agent.name.replace('-', '_'))}\n"
        f"description = {toml_string(agent.meta['description'])}\n"
        f"developer_instructions = {toml_multiline(agent.body)}\n"
    )


def build(out_root: Path) -> None:
    agents = load_agents()
    skills = validate_skills()
    for agent in agents:
        copilot = out_root / ".github" / "agents" / f"{agent.name}.agent.md"
        codex = out_root / ".codex" / "agents" / f"{agent.name}.toml"
        for target, content in ((copilot, render_copilot(agent)), (codex, render_codex(agent))):
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content, encoding="utf-8")
    for skill_dir in skills:
        shutil.copytree(skill_dir, out_root / ".agents" / "skills" / skill_dir.name)


GENERATED_DIRS = (Path(".github/agents"), Path(".codex/agents"), Path(".agents/skills"))


def snapshot(root: Path) -> dict[str, bytes]:
    files: dict[str, bytes] = {}
    for rel_dir in GENERATED_DIRS:
        base = root / rel_dir
        if base.exists():
            for path in base.rglob("*"):
                if path.is_file():
                    files[str(path.relative_to(root))] = path.read_bytes()
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="fail if generated files are out of date")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as tmp:
        tmp_root = Path(tmp)
        try:
            build(tmp_root)
        except ValueError as err:
            print(f"error: {err}", file=sys.stderr)
            return 1
        expected = snapshot(tmp_root)

        if args.check:
            actual = snapshot(ROOT)
            stale = sorted(set(expected) ^ set(actual) | {k for k in expected if actual.get(k) != expected[k]})
            if stale:
                print("Generated agent files are out of date. Run: python3 scripts/sync_agents.py", file=sys.stderr)
                for path in stale:
                    print(f"  {path}", file=sys.stderr)
                return 1
            print(f"OK: {len(expected)} generated files up to date.")
            return 0

        for rel_dir in GENERATED_DIRS:
            shutil.rmtree(ROOT / rel_dir, ignore_errors=True)
        for rel, content in expected.items():
            target = ROOT / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
        print(f"Wrote {len(expected)} generated files.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
