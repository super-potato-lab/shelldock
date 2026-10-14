#!/usr/bin/env python3
"""Validate installers and the portable ZIP before staging a release."""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tomllib
import zipfile


def main():
    source, destination = map(Path, sys.argv[1:])
    version = json.loads(Path("package.json").read_text())["version"]
    versions = {
        version,
        json.loads(Path("src-tauri/tauri.conf.json").read_text())["version"],
        tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"],
    }
    if len(versions) != 1 or not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise SystemExit("Rust, frontend and Tauri versions must match a valid release version")

    ref = os.environ["GITHUB_REF"]
    if ref.startswith("refs/tags/v"):
        tag = ref.removeprefix("refs/tags/")
        if tag != f"v{version}":
            raise SystemExit(f"Tag {tag} does not match application version v{version}")
        prerelease = "-" in version
    elif ref == "refs/heads/main":
        run = os.environ["GITHUB_RUN_NUMBER"]
        if not run.isdigit():
            raise SystemExit("Invalid workflow run number")
        tag = f"v{version}-preview.{run}"
        prerelease = True
    else:
        raise SystemExit("Releases are only allowed from main or matching version tags")

    packages = []
    for extension in (".exe", ".deb", ".zip"):
        matches = sorted(p for p in source.rglob(f"*{extension}") if p.is_file())
        if len(matches) != 1 or matches[0].stat().st_size == 0:
            raise SystemExit(f"Expected exactly one nonempty {extension} package")
        if extension == ".zip":
            if matches[0].name != f"ShellDock_{version}_windows_x64_portable.zip":
                raise SystemExit("Unexpected portable ZIP filename")
            try:
                with zipfile.ZipFile(matches[0]) as archive:
                    required = {"ShellDock.exe", "README.txt", "LICENSE", "THIRD_PARTY_NOTICES.md"}
                    if set(archive.namelist()) != required or archive.testzip() is not None:
                        raise SystemExit("Portable ZIP is corrupt or missing required files")
                    with archive.open("ShellDock.exe") as executable:
                        if executable.read(2) != b"MZ":
                            raise SystemExit("Portable ZIP does not contain a Windows executable")
            except zipfile.BadZipFile as error:
                raise SystemExit("Invalid portable ZIP") from error
        packages.extend(matches)
    if destination.exists():
        raise SystemExit("Release staging directory already exists")
    destination.mkdir(parents=True)
    checksums = []
    for package in packages:
        target = destination / package.name
        shutil.copyfile(package, target)
        with target.open("rb") as data:
            digest = hashlib.file_digest(data, "sha256").hexdigest()
        checksums.append(f"{digest}  {target.name}\n")
    (destination / "SHA256SUMS").write_text("".join(checksums))

    repository = os.environ["GITHUB_REPOSITORY"]
    sha = os.environ["GITHUB_SHA"]
    run_url = f"https://github.com/{repository}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
    Path("release-notes.md").write_text(
        "Windows x64 `.exe` 安装包、Windows x64 免安装 `.zip` 和 Linux x64 `.deb` 安装包。\n\n"
        "免安装版：解压后双击 `ShellDock.exe`；系统需有 Microsoft Edge WebView2 Runtime。\n\n"
        f"应用版本：`{version}`；源码提交：`{sha}`。\n\n"
        f"构建和测试：[GitHub Actions]({run_url})。\n\n"
        "`SHA256SUMS` 提供安装包的 SHA-256 校验值。\n\n"
        "当前为核心功能首版，兼容范围与限制见仓库 README。安装包尚未配置代码签名。\n",
        encoding="utf-8",
    )
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
        output.write(f"tag={tag}\nprerelease={str(prerelease).lower()}\n")
    print(json.dumps({"tag": tag, "prerelease": prerelease, "files": [p.name for p in packages]}))


if __name__ == "__main__":
    main()
