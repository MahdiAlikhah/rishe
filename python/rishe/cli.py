"""Command-line interface for Rishe."""

from __future__ import annotations

import argparse
import json
import sys
from importlib.metadata import PackageNotFoundError, version as _distribution_version
from pathlib import Path
from typing import List, Optional

from . import Embeddings, Pipeline, hub, parse_date
from .api import DEFAULT_MASKERS

# Kept for callers that imported the old name; the masker list itself lives in
# ``rishe.api`` so the API and the CLI cannot drift apart.
DEFAULT_MASK_STAGES = list(DEFAULT_MASKERS)


def _configure_stdio() -> None:
    """Emit and read UTF-8 regardless of the console code page."""
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8")


def _version() -> str:
    """Return the installed package version (``unknown`` for an uninstalled tree)."""
    try:
        return _distribution_version("rishe")
    except PackageNotFoundError:  # pragma: no cover - running from a bare source tree
        return "unknown"


def main(argv: Optional[List[str]] = None) -> int:
    """Run the CLI and return the process exit status.

    The statuses match the Rust binary; see ``docs/errors.md`` for the contract:
    ``0`` for success (including ``--help`` and ``--version``), ``2`` for usage
    errors that argparse rejects (unknown command, unknown or missing option
    value, wrong argument count), and ``1`` for every failure after a valid
    command line (missing/corrupt input, out-of-vocabulary word, pack or model
    load failure, network/cache error, ``serve`` startup failure).
    """
    _configure_stdio()
    parser = argparse.ArgumentParser(
        prog="rishe",
        description="Fast Persian NLP and morphology.",
    )
    parser.add_argument(
        "-V",
        "--version",
        action="version",
        version=f"%(prog)s {_version()}",
        help="show the version and exit",
    )
    subparsers = parser.add_subparsers(dest="command")

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
        "--maskers",
        "--stages",
        dest="maskers",
        nargs="+",
        default=list(DEFAULT_MASKERS),
        help="maskers to apply, in order (`--stages` is a deprecated alias)",
    )
    mask.add_argument("--mask-token", default=None, help="override every mask default")
    mask.add_argument("--json", action="store_true", help="emit JSON")

    date = subparsers.add_parser("date", help="Parse a Jalali date into Gregorian ISO.")
    date.add_argument("text", nargs="?", help="text; reads stdin when omitted")
    date.add_argument("--json", action="store_true", help="emit JSON")

    depparse = subparsers.add_parser(
        "depparse", help="Dependency parsing with a model pack."
    )
    depparse.add_argument("text", nargs="?", help="text; reads stdin when omitted")
    depparse.add_argument(
        "--pack", default=None, help="depparse pack directory (required at run time)"
    )
    depparse.add_argument("--json", action="store_true", help="emit JSON")

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
    serve.add_argument(
        "--maskers",
        "--stages",
        dest="maskers",
        nargs="+",
        default=list(DEFAULT_MASKERS),
    )
    serve.add_argument("--mask-token", default=None)
    serve.add_argument("--pos-pack", default=None)
    serve.add_argument("--ner-pack", default=None)
    serve.add_argument("--ezafe-pack", default=None)
    serve.add_argument("--depparse-pack", default=None)
    serve.add_argument("--model", default=None, help="embedding .vec path or hub model name")

    embed = subparsers.add_parser("embed", help="Mean embedding vector for text.")
    embed.add_argument("text", nargs="?", help="text; reads stdin when omitted")
    embed.add_argument("--model", default=None, help=".vec path or hub model name")
    embed.add_argument("--json", action="store_true", help="emit JSON")

    similar = subparsers.add_parser("similar", help="Nearest words to a word.")
    similar.add_argument("word")
    similar.add_argument("--top", type=int, default=5, help="neighbour count")
    similar.add_argument("--model", default=None, help=".vec path or hub model name")
    similar.add_argument("--json", action="store_true", help="emit JSON")

    similarity = subparsers.add_parser(
        "similarity", help="Cosine similarity between two words."
    )
    similarity.add_argument("first")
    similarity.add_argument("second")
    similarity.add_argument("--model", default=None, help=".vec path or hub model name")
    similarity.add_argument("--json", action="store_true", help="emit JSON")

    hub_parser = subparsers.add_parser("hub", help="Download and cache models.")
    hub_commands = hub_parser.add_subparsers(dest="hub_command", required=True)
    hub_pull = hub_commands.add_parser("pull", help="Download a model into the cache.")
    hub_pull.add_argument("name", help="model name or a URL with --sha256")
    hub_pull.add_argument("--sha256", default=None, help="required for custom URLs")
    hub_pull.add_argument("--force", action="store_true", help="re-download")
    hub_commands.add_parser("list", help="List known models and cache status.")
    hub_info = hub_commands.add_parser("info", help="Show cache information.")
    hub_info.add_argument("name")
    hub_path = hub_commands.add_parser("path", help="Print a cached model path.")
    hub_path.add_argument("name")
    hub_remove = hub_commands.add_parser("remove", help="Remove a cached model or pack.")
    hub_remove.add_argument("name")
    hub_verify = hub_commands.add_parser("verify", help="Verify cached checksums.")
    hub_verify.add_argument("name")

    args = parser.parse_args(argv)
    if args.command is None:
        parser.print_help()
        return 0
    if args.command == "hub":
        return _hub(args)
    if args.command == "date":
        return _date(args)
    if args.command == "depparse":
        return _depparse(args)
    if args.command in ("pos", "ner", "ezafe"):
        return _pack(args)

    if args.command in ("embed", "similar", "similarity"):
        return _embedding(args)

    if args.command == "serve":
        packs = {
            f"{task}_pack": getattr(args, f"{task}_pack")
            for task in ("pos", "ner", "ezafe", "depparse")
            if getattr(args, f"{task}_pack")
        }
        try:
            pipe = Pipeline(**packs)
            embeddings = _load_embeddings(args.model) if args.model else None
        except (OSError, ValueError, RuntimeError) as error:
            print(error, file=sys.stderr)
            return 1
        # Serve accepts both `--maskers url,email` (the Rust binary's form, and
        # the one documented in docs/serve-schema.md) and `--maskers url email`.
        maskers = [
            masker for value in args.maskers for masker in value.split(",") if masker
        ]
        return _serve(pipe, maskers, args.mask_token, embeddings)

    pipe = Pipeline()

    text = _read_text(args.text)
    try:
        if args.command == "normalize":
            result = pipe.normalize(text)
        elif args.command == "tokenize":
            result = pipe.tokenize(text)
        elif args.command == "lemmatize":
            result = pipe(text).lemmas
        elif args.command == "mask":
            result = pipe.mask(text, args.maskers, args.mask_token)
        else:
            # Unreachable: the required subparser rejects unknown commands with
            # SystemExit(2). Kept as a usage-error guard should the dispatch and
            # the parser ever drift apart.
            parser.error(f"unknown command: {args.command}")
    except (OSError, ValueError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(result, ensure_ascii=False))
    elif isinstance(result, str):
        print(result)
    else:
        print(" ".join(result))
    return 0


def _tag_rows(items, task):
    key = "label" if task == "ner" else "tag"
    return [
        {
            "text": item.text,
            key: getattr(item, key),
            "start": item.start,
            "end": item.end,
        }
        for item in items
    ]


def _date_payload(text: str) -> dict:
    """Return the Jalali/gregorian/ISO payload shared by the CLI and ``serve``.

    Raises ``ValueError`` when the text holds no usable Jalali date, so both
    entry points report the failure instead of drifting apart.
    """
    date = parse_date(text)
    if date is None:
        raise ValueError(f"cannot parse a Jalali date from {text!r}")
    gregorian = date.to_gregorian()
    iso = date.iso
    if iso is None or gregorian is None:
        raise ValueError(f"invalid Jalali date: {date!r}")
    return {
        "jalali": {
            "year": date.year,
            "month": date.month,
            "day": date.day,
        },
        "gregorian": {
            "year": gregorian[0],
            "month": gregorian[1],
            "day": gregorian[2],
        },
        "iso": iso,
    }


def _date(args) -> int:
    text = _read_text(args.text)
    try:
        payload = _date_payload(text)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(payload, ensure_ascii=False))
    else:
        print(payload["iso"])
    return 0


def _depparse(args) -> int:
    if not args.pack:
        print("depparse needs --pack <dir>", file=sys.stderr)
        return 1
    try:
        pipe = Pipeline(depparse_pack=args.pack)
        items = pipe.depparse(_read_text(args.text))
    except (OSError, ValueError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1

    if args.json:
        rows = [
            {
                "text": item.text,
                "head": item.head,
                "relation": item.relation,
                "start": item.start,
                "end": item.end,
            }
            for item in items
        ]
        print(json.dumps(rows, ensure_ascii=False))
    else:
        for item in items:
            head = "-" if item.head is None else str(item.head)
            print(f"{item.text}\t{head}\t{item.relation}")
    return 0


def _pack(args) -> int:
    task = args.command
    try:
        path = args.pack or hub.path(f"{task}-compact")
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    try:
        pipe = Pipeline(**{f"{task}_pack": path})
        items = getattr(pipe, task)(_read_text(args.text))
    except (OSError, ValueError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(_tag_rows(items, task), ensure_ascii=False))
    else:
        for item in items:
            label = item.label if task == "ner" else item.tag
            print(f"{item.text}\t{label}")
    return 0


def _hub(args) -> int:
    if args.hub_command == "pull":
        try:
            if args.name in hub.PACKS:
                print(hub.pull(args.name, force=args.force))
            else:
                print(hub.pull(args.name, sha256=args.sha256, force=args.force))
        except (OSError, ValueError, RuntimeError) as error:
            print(error, file=sys.stderr)
            return 1
    elif args.hub_command == "path":
        try:
            print(hub.path(args.name))
        except (OSError, ValueError) as error:
            print(error, file=sys.stderr)
            return 1
    elif args.hub_command == "info":
        try:
            info = hub.info(args.name)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 1
        if info is None:
            print(f"{args.name}\tmissing")
        else:
            print(f"{info['name']}\t{info['path']}\t{info['bytes']} bytes")
    elif args.hub_command == "remove":
        try:
            removed = hub.remove(args.name)
        except (OSError, ValueError) as error:
            print(error, file=sys.stderr)
            return 1
        print("removed" if removed else "not cached")
    elif args.hub_command == "verify":
        try:
            valid = hub.verify(args.name)
        except (OSError, ValueError) as error:
            print(error, file=sys.stderr)
            return 1
        print("verified" if valid else "invalid")
        return 0 if valid else 1
    else:
        for name, model in hub.MODELS.items():
            status = "cached" if hub.cached(name) else "missing"
            print(f"{name}\t{status}\t{model.filename}")
        for name in hub.PACKS:
            status = "cached" if hub.cached(name) else "missing"
            print(f"{name}\t{status}\tpack")
    return 0


def _load_embeddings(name: Optional[str]) -> Embeddings:
    value = name or "rishe-embed-fa-300-v2"
    direct = Path(value)
    if direct.is_file():
        return Embeddings(direct)
    if (
        value.endswith(".vec")
        or value.endswith(".rvec")
        or "/" in value
        or "\\" in value
    ):
        raise FileNotFoundError(f"embedding file not found: {value}")
    cached = hub.cached(value)
    if cached is None:
        raise FileNotFoundError(
            f"{value!r} is not cached; run `rishe hub pull {value}`"
        )
    return Embeddings(cached)


def _embedding(args) -> int:
    try:
        vectors = _load_embeddings(args.model)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1

    if args.command == "embed":
        vector = vectors.sentence_vector(_read_text(args.text))
        if args.json:
            print(json.dumps(vector))
        else:
            print(" ".join(f"{value:.6f}" for value in vector))
        return 0

    if args.command == "similar":
        if args.top < 1:
            print("--top must be at least 1", file=sys.stderr)
            return 1
        neighbours = vectors.most_similar(args.word, args.top)
        if not neighbours:
            print(f"{args.word!r} is not in the vocabulary", file=sys.stderr)
            return 1
        if args.json:
            rows = [{"word": word, "score": score} for word, score in neighbours]
            print(json.dumps(rows, ensure_ascii=False))
        else:
            for word, score in neighbours:
                print(f"{word}\t{score:.4f}")
        return 0

    score = vectors.similarity(args.first, args.second)
    if score is None:
        print(
            f"out of vocabulary: {args.first!r} or {args.second!r}",
            file=sys.stderr,
        )
        return 1
    if args.json:
        print(json.dumps({"score": score}))
    else:
        print(f"{score:.6f}")
    return 0


def _serve(
    pipe: Pipeline,
    stages: List[str],
    mask_token: Optional[str],
    embeddings: Optional[Embeddings] = None,
) -> int:
    """Answer protocol-1 JSON lines (`docs/serve-schema.md`) until stdin ends."""
    stream = getattr(sys.stdin, "buffer", sys.stdin)
    for raw in stream:
        if isinstance(raw, bytes):
            try:
                line = raw.decode("utf-8").strip()
            except UnicodeDecodeError:
                print(
                    json.dumps(
                        {"ok": False, "error": "request line is not valid UTF-8"},
                        ensure_ascii=False,
                    ),
                    flush=True,
                )
                continue
        else:
            line = raw.strip()
        if not line:
            continue
        try:
            try:
                request = json.loads(line)
            except ValueError as error:
                raise ValueError(f"invalid JSON: {error}") from error
            if not isinstance(request, dict):
                raise ValueError("request has no command")
            command = request.get("command")
            if not isinstance(command, str):
                raise ValueError("request has no command")
            text = request.get("text")
            if not isinstance(text, str):
                text = ""
            if command == "version":
                result = {"name": "rishe", "protocol": 1, "version": _version()}
            elif command == "normalize":
                result = pipe.normalize(text)
            elif command == "tokenize":
                result = pipe.tokenize(text)
            elif command == "lemmatize":
                result = pipe(text).lemmas
            elif command == "date":
                result = _date_payload(text)
            elif command == "mask":
                result = pipe.mask(
                    text,
                    request.get("maskers", request.get("stages", stages)),
                    request.get("mask_token", mask_token),
                )
            elif command == "depparse":
                result = [
                    {
                        "text": item.text,
                        "head": item.head,
                        "relation": item.relation,
                        "start": item.start,
                        "end": item.end,
                    }
                    for item in pipe.depparse(text)
                ]
            elif command in ("pos", "ner", "ezafe"):
                items = getattr(pipe, command)(text)
                result = _tag_rows(items, command)
            elif command in ("embed", "similar", "similarity"):
                if embeddings is None:
                    raise ValueError("serve has no embedding model; pass --model")
                if command == "embed":
                    result = embeddings.sentence_vector(text)
                elif command == "similar":
                    word = request.get("word", text)
                    result = [
                        {"word": candidate, "score": score}
                        for candidate, score in embeddings.most_similar(
                            word, int(request.get("n", 5))
                        )
                    ]
                else:
                    first = request.get("a")
                    if not isinstance(first, str):
                        first = text
                    second = request.get("b")
                    if not isinstance(second, str):
                        raise ValueError("similarity needs `a` and `b`")
                    score = embeddings.similarity(first, second)
                    if score is None:
                        raise ValueError(
                            f"out of vocabulary: {first!r} or {second!r}"
                        )
                    result = {"score": score}
            else:
                raise ValueError(f"unknown command: {command}")
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
