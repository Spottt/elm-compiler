#!/usr/bin/env python3
"""Replay the differential checks against an official Elm release other than 0.19.1.

The checks build fixtures for Elm 0.19.1: manifests pin "0.19.1" and package
caches live under ELM_HOME/0.19.1. This module wraps both compilers so the
same fixtures are compiled as applications of another release:

  * the manifest in the working directory is pinned to that release for the
    duration of one command, then restored byte for byte with its timestamps;
  * for the official binary, package sources cached for 0.19.1 are mirrored
    into the directory of that release, with a registry in its format;
  * the release number is rewritten to 0.19.1 in captured output, so guide
    links and banners compare equal.

Limits: checks that edit or inspect the 0.19.1 package cache themselves
(install, diff, publish, download and cache suites), and checks driving a
terminal, cannot be judged this way. POSIX only (symbolic links, shebangs).
"""
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys

PINNED = re.compile(rb'("elm-version"\s*:\s*")0\.19\.1(")')
# Long-lived protocols must stream; everything else is captured unless a terminal is attached.
STREAMING = {'--make-worker', '--internal-make-worker'}


def mirror_package_cache(release, registry_home):
    """Give the official binary the package sources a check prepared for 0.19.1."""
    home = os.environ.get('ELM_HOME')
    if not home:
        return
    source, target = Path(home, '0.19.1/packages'), Path(home, release)
    if not source.is_dir() or target.exists():
        return
    shutil.copytree(source, target / 'packages', ignore=shutil.ignore_patterns('artifacts.dat', 'registry.dat'))
    registry = Path(registry_home, release, 'packages/registry.dat')
    if (source / 'registry.dat').exists() and registry.exists():
        shutil.copyfile(registry, target / 'packages/registry.dat')


def run(real, release, official, registry_home):
    arguments = sys.argv[1:]
    if official:
        mirror_package_cache(release, registry_home)
    manifest, original = Path('elm.json'), None
    if manifest.is_file():
        content = manifest.read_bytes()
        pinned = PINNED.sub(lambda match: match[1] + release.encode() + match[2], content)
        if pinned != content:
            times = manifest.stat()
            original = (content, (times.st_atime_ns, times.st_mtime_ns))
            manifest.write_bytes(pinned)
            os.utime(manifest, ns=original[1])
    try:
        interactive = (arguments[:1] and arguments[0] in STREAMING) or sys.stdout.isatty() or sys.stderr.isatty()
        if interactive:
            code = subprocess.call([real, *arguments])
            output = None
        else:
            result = subprocess.run([real, *arguments], stdin=sys.stdin, capture_output=True)
            code, output = result.returncode, (result.stdout, result.stderr)
    finally:
        if original:
            manifest.write_bytes(original[0])
            os.utime(manifest, ns=original[1])
    if output:
        for stream, data in zip((sys.stdout, sys.stderr), output):
            stream.buffer.write(data.replace(release.encode(), b'0.19.1'))
            stream.flush()
    # A crash of the wrapped compiler stays visible as the shell would report it.
    raise SystemExit(128 - code if code < 0 else code)


def write_wrapper(path, real, release, official, registry_home):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        f'#!{sys.executable}\n'
        'import sys\n'
        f'sys.path.insert(0, {str(Path(__file__).resolve().parent)!r})\n'
        'import elm_release_adapter\n'
        f'elm_release_adapter.run({str(real)!r}, {release!r}, {official!r}, {str(registry_home)!r})\n')
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return path


def mirror(crate, output, elm, binary, release):
    """Build a copy of the crate layout whose two compilers behave as `release`.

    Returns (scripts directory, wrapped compiler, wrapped official binary). Checks
    locate the compiler relative to their own path, hence the mirrored layout.
    """
    root = output / 'release-mirror'
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True)
    shutil.copytree(crate / 'scripts', root / 'scripts', ignore=shutil.ignore_patterns('__pycache__'))
    for entry in crate.iterdir():
        if entry.name not in ('scripts', 'target', '.git'):
            (root / entry.name).symlink_to(entry)
    registry_home = Path(os.environ.get('ELM_HOME', Path.home() / '.elm'))
    wrapped = write_wrapper(root / 'target/release/planexpo-elm', binary, release, False, registry_home)
    official = write_wrapper(root / 'official/elm', elm, release, True, registry_home)
    return root / 'scripts', wrapped, official
