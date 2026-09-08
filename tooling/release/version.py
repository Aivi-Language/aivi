"""Validate release tags against the committed Cargo workspace and lockfile."""

import argparse
import os
from pathlib import Path
import re
import tomllib


# Build metadata is valid SemVer, but deliberately excluded from release tags:
# it does not change precedence and would permit multiple tags for one version.
TAG = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?", re.ASCII)


def release_version(tag):
    match = TAG.fullmatch(tag)
    if not match:
        raise ValueError("release tag must be vMAJOR.MINOR.PATCH[-PRERELEASE], without build metadata")
    prerelease = match.group(4)
    if prerelease:
        for identifier in prerelease.split("."):
            if identifier.isdigit() and len(identifier) > 1 and identifier.startswith("0"):
                raise ValueError("numeric prerelease identifiers must not have leading zeroes")
    return tag[1:], prerelease is not None


def validate_workspace(root, tag):
    version, prerelease = release_version(tag)
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    workspace = manifest["workspace"]
    if workspace["package"]["version"] != version:
        raise ValueError(f"{tag} does not match workspace.package.version")
    locked = tomllib.loads((root / "Cargo.lock").read_text())
    local_packages = {(p["name"], p["version"]) for p in locked["package"] if "source" not in p}
    for member in workspace["members"]:
        paths = sorted(root.glob(member))
        if not paths:
            raise ValueError(f"workspace member has no matching directory: {member}")
        for member_path in paths:
            package = tomllib.loads((member_path / "Cargo.toml").read_text())["package"]
            if package.get("version") != {"workspace": True}:
                raise ValueError(f"{package['name']} must inherit the workspace release version")
            if (package["name"], version) not in local_packages:
                raise ValueError(f"Cargo.lock is stale for {package['name']} {version}")
    return version, prerelease


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    try:
        version, prerelease = validate_workspace(args.root, args.tag)
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"release validation failed: {error}\n")
    output = f"version={version}\nprerelease={str(prerelease).lower()}\n"
    print(output, end="")
    if destination := os.environ.get("GITHUB_OUTPUT"):
        with open(destination, "a", encoding="utf-8") as stream:
            stream.write(output)


if __name__ == "__main__":
    main()
