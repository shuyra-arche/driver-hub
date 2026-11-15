"""Decide whether a NUL-delimited Git path list contains only Markdown files."""

import sys


def is_docs_only(output: bytes) -> bool:
    """Return true only for a non-empty, complete list of .md paths."""
    if not output or not output.endswith(b"\0"):
        return False
    paths = output[:-1].split(b"\0")
    return bool(paths) and all(path.endswith(b".md") for path in paths)


def main() -> None:
    run_handoff = not is_docs_only(sys.stdin.buffer.read())
    print(f"run_handoff={str(run_handoff).lower()}")


if __name__ == "__main__":
    main()
