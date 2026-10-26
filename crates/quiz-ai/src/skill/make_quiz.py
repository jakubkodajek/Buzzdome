#!/usr/bin/env python3
"""Checks a Buzzdome quiz folder and packs it into a .quiz file.

A quiz folder holds quiz.json and the pictures, sounds and videos it uses in
media/. This script checks quiz.json against the format's JSON Schema
(reference/quiz.schema.json next to this script's folder) and that every
media file it names is there, then writes the .quiz file: a ZIP with
quiz.json and the media it uses. Buzzdome checks the rest when it opens the
file and lists anything left to fix under File > Info.

Usage: python3 make_quiz.py QUIZ_FOLDER [-o OUTPUT.quiz]

Only the standard library is used, so it runs without installing anything.
"""

import argparse
import json
import re
import sys
import zipfile
from pathlib import Path

SCHEMA = Path(__file__).resolve().parent.parent / "reference" / "quiz.schema.json"
MEDIA_PREFIX = "media/"
# Characters some system refuses in a file name, besides control characters.
FORBIDDEN_IN_NAMES = set('\\/:*?"<>|')
# Device names Windows reserves with any extension.
RESERVED_NAMES = {"CON", "PRN", "AUX", "NUL"} | {f"{device}{n}" for device in ("COM", "LPT") for n in "0123456789¹²³"}


def json_type(value):
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if value is None:
        return "null"
    if isinstance(value, list):
        return "array"
    return "object"


class Checker:
    """The parts of JSON Schema that the quiz schema uses."""

    def __init__(self, schema):
        self.schema = schema
        self.defs = schema.get("$defs", {})

    def check(self, quiz):
        return self.errors(quiz, self.schema, "quiz")

    def errors(self, value, schema, path):
        if "$ref" in schema:
            # The definition and what the schema says beside it both apply.
            rest = {k: v for k, v in schema.items() if k != "$ref"}
            return self.errors(value, self.defs[schema["$ref"].rsplit("/", 1)[-1]], path) + self.errors(value, rest, path)
        expected = schema.get("type")
        if expected is not None:
            allowed = expected if isinstance(expected, list) else [expected]
            actual = json_type(value)
            if actual not in allowed and not (actual == "integer" and "number" in allowed):
                return [f"{path}: expected {' or '.join(allowed)}, found {actual}"]
        if "const" in schema and value != schema["const"]:
            return [f"{path}: must be {json.dumps(schema['const'])}"]
        if "enum" in schema and value not in schema["enum"]:
            return [f"{path}: must be one of {', '.join(json.dumps(v) for v in schema['enum'])}"]
        found = []
        if isinstance(value, str) and "pattern" in schema and not re.search(schema["pattern"], value):
            found.append(f"{path}: must match {schema['pattern']}")
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            if "minimum" in schema and value < schema["minimum"]:
                found.append(f"{path}: must be at least {schema['minimum']}")
            if "maximum" in schema and value > schema["maximum"]:
                found.append(f"{path}: must be at most {schema['maximum']}")
        if isinstance(value, dict):
            found += [f"{path}: missing {name}" for name in schema.get("required", []) if name not in value]
            for name, field in schema.get("properties", {}).items():
                if name in value:
                    found += self.errors(value[name], field, f"{path}.{name}")
        if isinstance(value, list) and "items" in schema:
            for i, item in enumerate(value):
                found += self.errors(item, schema["items"], f"{path}[{i}]")
        if "anyOf" in schema:
            failures = [self.errors(value, choice, path) for choice in schema["anyOf"]]
            if all(failures):
                # An optional field (one form or null) says why the value is not that form.
                forms = [failed for choice, failed in zip(schema["anyOf"], failures) if choice.get("type") != "null"]
                found += forms[0] if len(forms) == 1 else [f"{path}: does not match any allowed form"]
        if "oneOf" in schema:
            found += self.one_of(value, schema["oneOf"], path)
        return found

    def one_of(self, value, choices, path):
        # A slide's "type" says which form it has: it is checked against that one.
        tags = [choice.get("properties", {}).get("type", {}).get("const") for choice in choices]
        if isinstance(value, dict) and all(tags):
            if value.get("type") in tags:
                return self.errors(value, choices[tags.index(value["type"])], path)
            return [f"{path}.type: must be one of {', '.join(json.dumps(tag) for tag in tags)}"]
        matching = [choice for choice in choices if not self.errors(value, choice, path)]
        return [] if len(matching) == 1 else [f"{path}: does not match exactly one allowed form"]


def media_references(value):
    """Every media file the quiz names, in order of appearance: every text
    that starts with media/ (or media\\, as Windows writes it), whichever
    field holds it."""
    if isinstance(value, str):
        if value.startswith((MEDIA_PREFIX, "media\\")):
            yield value
    elif isinstance(value, dict):
        for field in value.values():
            yield from media_references(field)
    elif isinstance(value, list):
        for item in value:
            yield from media_references(item)


def is_media_name(reference):
    """Whether `reference` is media/<file name> with a name every system
    stores, and so a file inside the media folder: no folder, no character
    some system refuses, no trailing dot or space (which rules out . and ..),
    and no Windows device name."""
    name = reference[len(MEDIA_PREFIX):]
    return (
        name != ""
        and not name.endswith((".", " "))
        and not any(char in FORBIDDEN_IN_NAMES or ord(char) < 32 or 127 <= ord(char) < 160 for char in name)
        and name.split(".")[0].rstrip().upper() not in RESERVED_NAMES
    )


def check(folder):
    """The quiz in `folder` and what is wrong with it."""
    try:
        quiz = json.loads((folder / "quiz.json").read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None, [f"{folder / 'quiz.json'} is missing"]
    except (ValueError, UnicodeDecodeError) as error:
        return None, [f"quiz.json is not valid UTF-8 JSON: {error}"]
    found = Checker(json.loads(SCHEMA.read_text(encoding="utf-8"))).check(quiz)
    for reference in dict.fromkeys(media_references(quiz)):
        if not is_media_name(reference):
            found.append(f"{reference}: name media files media/<file name>, without folders or the characters \\ / : * ? \" < > |")
        elif not (folder / reference).is_file():
            found.append(f"{reference}: the file is missing from {folder / 'media'}")
    return quiz, found


def pack(folder, quiz, output):
    """Writes the quiz file: quiz.json compressed, media stored as they are."""
    with zipfile.ZipFile(output, "w") as archive:
        archive.writestr("quiz.json", json.dumps(quiz, ensure_ascii=False, indent=2), compress_type=zipfile.ZIP_DEFLATED)
        for reference in dict.fromkeys(media_references(quiz)):
            archive.write(folder / reference, reference, compress_type=zipfile.ZIP_STORED)


def main():
    parser = argparse.ArgumentParser(description="Check a Buzzdome quiz folder and pack it into a .quiz file.")
    parser.add_argument("folder", type=Path, help="the folder with quiz.json and media/")
    parser.add_argument("-o", "--output", type=Path, help="the .quiz file to write (default: the folder's name)")
    args = parser.parse_args()
    quiz, found = check(args.folder)
    if found:
        print("The quiz is not ready:", file=sys.stderr)
        for problem in found:
            print(f"- {problem}", file=sys.stderr)
        return 1
    output = args.output or args.folder.with_suffix(".quiz")
    if output.suffix.lower() != ".quiz":
        output = output.with_name(output.name + ".quiz")
    pack(args.folder, quiz, output)
    questions = sum(1 for slide in quiz["slides"] if slide.get("type") == "question")
    print(f"Wrote {output}: {len(quiz['slides'])} slides, {questions} questions.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
