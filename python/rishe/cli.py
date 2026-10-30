"""Command-line interface for Rishe."""

from __future__ import annotations

import argparse
import json
import sys
from typing import List, Optional

from . import Pipeline, hub

DEFAULT_MASK_STAGES = [
    "url",
    "email",
    "mention",
    "hashtag",
    "emoji",
    "html",
    "non_persian",
    "digits",
    "diacritics",
    "punctuation",
]


def _configure_stdio() -> None:
    """Emit and read UTF-8 regardless of the console code page."""
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8")


def main(argv: Optional[List[str]] = None) -> int:
    _configure_stdio()
    parser = argparse.ArgumentParser(
        prog="rishe",
        description="Fast Persian NLP and morphology.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    for name, description in (
        ("normalize", "Normalize Persian text."),
        ("tokenize", "Tokenize Persian text."),
        ("lemmatize", "Lemmatize Persian text."),
    ):
        command = subparsers.add_parser(name, help=description)
        command.add_argument("text", nargs="?", help="text; reads stdin when omitted")
        command.add_argument("--json", action="store_true", help="emit JSON")

    mask = subparsers.add_parser("mask", help="Mask or remove sensitive tokens.")
    mask.add_argument("text", nargs="?", help="text; reads stdin when omitted")
    mask.add_argument(
        "--stages",
        nargs="+",
        default=DEFAULT_MASK_STAGES,
        help="maskers to apply, in order",
    )
    mask.add_argument("--mask-token", default=None, help="override every mask default")
    mask.add_argument("--json", action="store_true", help="emit JSON")

    for name, description in (
        ("pos", "Part-of-speech tagging with a model pack."),
        ("ner", "Named-entity recognition with a model pack."),
        ("ezafe", "Ezafe tagging with a model pack."),
    ):
        command = subparsers.add_parser(name, help=description)
        command.add_argument("text", nargs="?", help="text; reads stdin when omitted")
        command.add_argument("--pack", default=None, help="pack directory; defaults to the Hub cache")
        command.add_argument("--json", action="store_true", help="emit JSON")

    serve = subparsers.add_parser("serve", help="JSON-lines server on stdin/stdout.")
    serve.add_argument("--stages", nargs="+", default=DEFAULT_MASK_STAGES)
    serve.add_argument("--mask-token", default=None)
    serve.add_argument("--pos-pack", default=None)
    serve.add_argument("--ner-pack", default=None)
    serve.add_argument("--ezafe-pack", default=None)

    hub_parser = subparsers.add_parser("hub", help="Download and cache models.")
    hub_commands = hub_parser.add_subparsers(dest="hub_command", required=True)
    hub_pull = hub_commands.add_parser("pull", help="Download a model into the cache.")
    hub_pull.add_argument("name", help="model name or a URL with --sha256")
    hub_pull.add_argument("--sha256", default=None, help="required for custom URLs")
    hub_pull.add_argument("--force", action="store_true", help="re-download")
    hub_commands.add_parser("list", help="List known models and cache status.")
    hub_info = hub_commands.add_parser("info", help="Show cache information.")
    hub_info.add_argument("name")
    hub_remove = hub_commands.add_parser("remove", help="Remove a cached model or pack.")
    hub_remove.add_argument("name")
    hub_verify = hub_commands.add_parser("verify", help="Verify cached checksums.")
    hub_verify.add_argument("name")

    args = parser.parse_args(argv)
    if args.command == "hub":
        return _hub(args)
    if args.command in ("pos", "ner", "ezafe"):
        return _pack(args)

    if args.command == "serve":
        packs = {
            f"{task}_pack": getattr(args, f"{task}_pack")
            for task in ("pos", "ner", "ezafe")
            if getattr(args, f"{task}_pack")
        }
        try:
            pipe = Pipeline(**packs)
        except (OSError, ValueError) as error:
            print(error, file=sys.stderr)
            return 2
        return _serve(pipe, args.stages, args.mask_token)

    pipe = Pipeline()

    text = _read_text(args.text)
    if args.command == "normalize":
        result = pipe.normalize(text)
    elif args.command == "tokenize":
        result = pipe.word_tokenize(text)
    elif args.command == "lemmatize":
        result = pipe(text).lemmas
    elif args.command == "mask":
        result = pipe.mask_stages(text, args.stages, args.mask_token)
    else:
        parser.error(f"unknown command: {args.command}")
        return 2

    if args.json:
        print(json.dumps(result, ensure_ascii=False))
    elif isinstance(result, str):
        print(result)
    else:
        print(" ".join(result))
    return 0


def _pack(args) -> int:
    task = args.command
    try:
        path = args.pack or hub.path_pack(f"{task}-compact")
    except FileNotFoundError as error:
        print(error, file=sys.stderr)
        return 2
    try:
        pipe = Pipeline(**{f"{task}_pack": path})
        items = getattr(pipe, task)(_read_text(args.text))
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2

    if args.json:
        rows = []
        for item in items:
            row = {"text": item.text, "start": item.start, "end": item.end}
            if task == "ner":
                row["label"] = item.label
            else:
                row["tag"] = item.tag
            rows.append(row)
        print(json.dumps(rows, ensure_ascii=False))
    else:
        for item in items:
            label = item.label if task == "ner" else item.tag
            print(f"{item.text}\t{label}")
    return 0


def _hub(args) -> int:
    if args.hub_command == "pull":
        if args.name in hub.PACKS:
            print(hub.pull_pack(args.name, force=args.force))
        else:
            print(hub.pull(args.name, sha256=args.sha256, force=args.force))
    elif args.hub_command == "info":
        try:
            info = hub.info(args.name)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        if info is None:
            print(f"{args.name}\tmissing")
        else:
            print(f"{info['name']}\t{info['path']}\t{info['bytes']} bytes")
    elif args.hub_command == "remove":
        try:
            removed = hub.remove(args.name)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        print("removed" if removed else "not cached")
    elif args.hub_command == "verify":
        try:
            valid = hub.verify(args.name)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        print("verified" if valid else "missing or corrupt")
        return 0 if valid else 1
    else:
        for name, model in hub.MODELS.items():
            status = "cached" if hub.cached(name) else "missing"
            print(f"{name}\t{status}\t{model.filename}")
        for name in hub.PACKS:
            status = "cached" if hub.cached_pack(name) else "missing"
            print(f"{name}\t{status}\tpack")
    return 0


def _serve(pipe: Pipeline, stages: List[str], mask_token: Optional[str]) -> int:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            command = request.get("command") or request.get("cmd")
            text = request.get("text", "")
            if command == "normalize":
                result = pipe.normalize(text)
            elif command == "tokenize":
                result = pipe.word_tokenize(text)
            elif command == "lemmatize":
                result = pipe(text).lemmas
            elif command == "mask":
                result = pipe.mask_stages(
                    text,
                    request.get("stages", stages),
                    request.get("mask_token", mask_token),
                )
            elif command in ("pos", "ner", "ezafe"):
                items = getattr(pipe, command)(text)
                if command == "ner":
                    result = [
                        {
                            "text": item.text,
                            "label": item.label,
                            "start": item.start,
                            "end": item.end,
                        }
                        for item in items
                    ]
                else:
                    result = [
                        {
                            "text": item.text,
                            "tag": item.tag,
                            "start": item.start,
                            "end": item.end,
                        }
                        for item in items
                    ]
            else:
                raise ValueError(f"unknown command: {command!r}")
            response = {"ok": True, "result": result}
        except Exception as error:  # keep the server alive on bad requests
            response = {"ok": False, "error": str(error)}
        print(json.dumps(response, ensure_ascii=False), flush=True)
    return 0


def _read_text(source: Optional[str]) -> str:
    if source is None or source == "-":
        return sys.stdin.read().strip()
    return source


if __name__ == "__main__":
    raise SystemExit(main())
