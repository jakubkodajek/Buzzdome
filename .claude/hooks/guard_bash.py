#!/usr/bin/env python3
"""PreToolUse hook for Bash: a speed bump against destructive commands.

Reads the hook's JSON on stdin and exits 2 (block, stderr shown to the agent)
when a command would push a release tag, force-push, delete a root, home or
the whole working directory, drop all local work, skip git hooks, force-add
ignored files (the reference manual copies) or pipe a download into a shell.

It looks at the words of each simple command (split on `&&`, `||`, `;`, `|`,
`&` and newlines), not at substrings of the whole line, so a PR title such as
"UX pass for v0.8.0" or a body with "+3627" is not taken for a push. It is
not a sandbox: the real guards are CI's checks, `.gitignore` and the
`release` environment's approval (docs/releasing.md).
"""

from __future__ import annotations

import json
import os
import re
import shlex
import sys

SEPARATORS = {"&&", "||", ";", "|", "&", "|&", ";;", "\n"}
SHELLS = {"sh", "bash", "zsh", "dash", "fish", "python", "python3", "node"}
PREFIXES = {"sudo", "command", "exec", "nohup", "time", "builtin"}
# Places whose recursive delete is never wanted.
ROOT_TARGETS = {"/", "/*", "~", "~/", "~/*", "$HOME", "$HOME/", "$HOME/*", "${HOME}", "${HOME}/", ".", "./", "./*", "*", "..", "../"}
TAG_REF = re.compile(r"^(?:\+)?(?:refs/tags/|v\d)")


def block(reason: str) -> None:
    print(f"Blocked by .claude/hooks/guard_bash.py: {reason}", file=sys.stderr)
    print("If this is really needed, ask Jakub to run it.", file=sys.stderr)
    sys.exit(2)


def split_commands(command: str) -> list[list[list[str]]]:
    """Return pipelines, each a list of simple commands (lists of words)."""
    lexer = shlex.shlex(command.replace("\n", " ; "), posix=True, punctuation_chars=";&|")
    lexer.whitespace_split = True
    lexer.commenters = ""
    try:
        tokens = list(lexer)
    except ValueError:
        # Unbalanced quotes: fall back to plain whitespace words.
        tokens = command.split()
    pipelines: list[list[list[str]]] = [[[]]]
    for token in tokens:
        if token in ("|", "|&"):
            pipelines[-1].append([])
        elif token in SEPARATORS:
            pipelines.append([[]])
        else:
            pipelines[-1][-1].append(token)
    return [[words for words in pipeline if words] for pipeline in pipelines if any(pipeline)]


def program(words: list[str]) -> list[str]:
    """Drop leading `VAR=value` assignments and wrappers such as sudo or env."""
    i = 0
    while i < len(words):
        word = words[i]
        if re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", word) or word in PREFIXES:
            i += 1
        elif word == "env":
            i += 1
            while i < len(words) and (words[i].startswith("-") or "=" in words[i]):
                i += 1
        else:
            break
    words = words[i:]
    if words:
        words = [os.path.basename(words[0]), *words[1:]]
    return words


def git_subcommand(words: list[str]) -> tuple[str, list[str]]:
    """Skip git's global options (`-C dir`, `-c k=v`, `--git-dir=...`)."""
    i = 1
    while i < len(words) and words[i].startswith("-"):
        i += 2 if words[i] in ("-C", "-c", "--git-dir", "--work-tree", "--namespace") else 1
    if i >= len(words):
        return "", []
    return words[i], words[i + 1 :]


def short_flags(args: list[str]) -> set[str]:
    return {c for a in args if re.match(r"^-[A-Za-z]+$", a) for c in a[1:]}


def check_rm(args: list[str]) -> None:
    flags = short_flags(args)
    recursive = "r" in flags or "R" in flags or "--recursive" in args
    if recursive and any(a.rstrip() in ROOT_TARGETS for a in args if not a.startswith("-")):
        block("recursive delete of a root, home or the whole working directory")


def check_git(words: list[str]) -> None:
    sub, args = git_subcommand(words)
    if sub == "push":
        if "--force" in args or "f" in short_flags(args) or "--mirror" in args:
            block("force push (use --force-with-lease on your own branch if you must)")
        if "--tags" in args or "--follow-tags" in args:
            block("pushing tags: a v* tag publishes a release to every installed app; only Jakub pushes one (docs/releasing.md)")
        refspecs = [a for a in args if not a.startswith("-")][1:]
        for ref in refspecs:
            if ref.startswith("+"):
                block("force push (a refspec starting with +)")
            src, _, dst = ref.partition(":")
            if TAG_REF.match(src.lstrip("+")) or TAG_REF.match(dst):
                block("a v* tag publishes a release to every installed app; only Jakub pushes one (docs/releasing.md)")
        if "--no-verify" in args:
            block("skipping git hooks")
    elif sub == "commit" and "--no-verify" in args:
        block("skipping git hooks")
    elif sub == "reset" and "--hard" in args and any(a.startswith(("origin", "upstream")) for a in args):
        block("discarding all local work")
    elif sub == "clean" and {"f", "d", "x"} <= short_flags(args):
        block("discarding all local work, ignored files included")
    elif sub == "add":
        if "--force" in args or "f" in short_flags(args):
            block("force-adding ignored files (the reference manual copies must never be committed)")
        if any(a.lstrip("./").startswith(("replica/sources", "replica/screens", "replica/captures")) for a in args):
            block("adding the reference manual copies or captures (clean room)")


def check_gh(words: list[str]) -> None:
    args = words[1:]
    if args[:1] == ["release"] and args[1:2] and args[1] not in ("list", "view", "download"):
        block("releases are made only by release.yml from Jakub's tag (docs/releasing.md)")
    if args[:1] == ["api"]:
        writes = any(a in ("-f", "-F", "--field", "--raw-field", "--input") for a in args)
        method = next((args[i + 1].upper() for i, a in enumerate(args[:-1]) if a in ("-X", "--method")), "GET")
        if (writes or method != "GET") and any("refs/tags" in a or "/releases" in a or "/git/tags" in a for a in args):
            block("creating a tag or release through the API; only Jakub releases (docs/releasing.md)")


def check(command: str) -> None:
    for pipeline in split_commands(command):
        names = []
        for raw in pipeline:
            words = program(raw)
            if not words:
                continue
            name = words[0]
            names.append(name)
            if name == "rm":
                check_rm(words[1:])
            elif name == "git":
                check_git(words)
            elif name == "gh":
                check_gh(words)
            elif name == "chmod" and "-R" in words and "777" in words:
                block("dangerous system command")
            elif name.startswith("mkfs") or (name == "dd" and any(a.startswith("of=/dev/") for a in words)):
                block("dangerous system command")
        for left, right in zip(names, names[1:]):
            if left in ("curl", "wget") and right in SHELLS:
                block("piping a download into a shell")


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except ValueError:
        block("the hook's input is not JSON")
    command = (data.get("tool_input") or {}).get("command") or ""
    check(command)


if __name__ == "__main__":
    main()
