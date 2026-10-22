"""Command-line interface for Rishe."""

from __future__ import annotations

import argparse
import json
import sys
from typing import List, Optional

from . import Pipeline

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


def main(argv: Optional[List[str]] = None) -> int:
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

    serve = subparsers.add_parser("serve", help="JSON-lines server on stdin/stdout.")
    serve.add_argument("--stages", nargs="+", default=DEFAULT_MASK_STAGES)
    serve.add_argument("--mask-token", default=None)

    args = parser.parse_args(argv)
    pipe = Pipeline()

    if args.command == "serve":
        return _serve(pipe, args.stages, args.mask_token)

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
