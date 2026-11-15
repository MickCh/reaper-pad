#!/usr/bin/env python3
"""Add a release to the ReaPack index (index.xml).

Usage:
    scripts/reapack_index.py VERSION [--time ISO8601] [--index index.xml]

The changelog for the version is taken from CHANGELOG.md ("## [VERSION]"
section). Download URLs point to the GitHub release "vVERSION", which must
contain the files listed in PLATFORMS. Only the Python standard library is
used, so this runs anywhere (including GitHub Actions) without setup.
"""

import argparse
import datetime as dt
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

REPO = "MickCh/reaper-pad"
INDEX_NAME = "reaper-pad"
PACKAGE = "reaper-pad.ext"
CATEGORY = "Extensions"
DESC = "reaper-pad: use a USB gamepad as a remote control for REAPER"
AUTHOR = "MickCh"
WEBSITE = f"https://github.com/{REPO}"

# ReaPack platform -> file name (installed into REAPER's UserPlugins folder)
PLATFORMS = {
    "win64": "reaper_pad-x64.dll",
    "linux64": "reaper_pad-x86_64.so",
}

ROOT = Path(__file__).resolve().parent.parent


def changelog_for(version: str, changelog: Path) -> str:
    text = changelog.read_text(encoding="utf-8")
    m = re.search(
        rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|\Z)", text, re.M | re.S
    )
    if not m:
        sys.exit(f"error: no '## [{version}]' section in {changelog}")
    body = m.group(1).strip()
    # ReaPack shows plain text: "### Added" → "Added:", `code` → code
    body = re.sub(r"^### (.+)$", r"\1:", body, flags=re.M)
    return body.replace("`", "")


def new_index() -> ET.Element:
    root = ET.Element("index", version="1", name=INDEX_NAME)
    ET.SubElement(root, "category", name=CATEGORY)
    meta = ET.SubElement(root, "metadata")
    link = ET.SubElement(meta, "link", rel="website", href=WEBSITE)
    link.text = "Website"
    return root


def package(root: ET.Element) -> ET.Element:
    cat = root.find(f"category[@name='{CATEGORY}']")
    if cat is None:
        cat = ET.SubElement(root, "category", name=CATEGORY)
    pkg = cat.find(f"reapack[@name='{PACKAGE}']")
    if pkg is None:
        pkg = ET.SubElement(cat, "reapack", name=PACKAGE, type="extension", desc=DESC)
        meta = ET.SubElement(pkg, "metadata")
        ET.SubElement(meta, "link", rel="website").text = WEBSITE
    return pkg


def add_version(root: ET.Element, version: str, time: str, changelog: str) -> None:
    pkg = package(root)
    if pkg.find(f"version[@name='{version}']") is not None:
        sys.exit(f"error: version {version} is already in the index")
    ver = ET.SubElement(pkg, "version", name=version, author=AUTHOR, time=time)
    ET.SubElement(ver, "changelog").text = changelog
    base = f"https://github.com/{REPO}/releases/download/v{version}"
    for platform, file in PLATFORMS.items():
        src = ET.SubElement(ver, "source", platform=platform, file=file)
        src.text = f"{base}/{file}"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("version", help="version without the leading v, e.g. 0.2.0")
    ap.add_argument("--time", help="release time (ISO 8601, UTC); default: now")
    ap.add_argument("--index", type=Path, default=ROOT / "index.xml")
    ap.add_argument("--changelog", type=Path, default=ROOT / "CHANGELOG.md")
    args = ap.parse_args()

    version = args.version.removeprefix("v")
    if not re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?", version):
        sys.exit(f"error: invalid version '{args.version}'")
    time = args.time or dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")

    root = ET.parse(args.index).getroot() if args.index.exists() else new_index()
    add_version(root, version, time, changelog_for(version, args.changelog))
    ET.indent(root, space="  ")
    tree = ET.ElementTree(root)
    tree.write(args.index, encoding="utf-8", xml_declaration=True)
    with open(args.index, "a", encoding="utf-8") as f:
        f.write("\n")
    print(f"{args.index}: added {PACKAGE} {version}")


if __name__ == "__main__":
    main()
