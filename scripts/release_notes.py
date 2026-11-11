#!/usr/bin/env python3
"""Build short GitHub release notes for one version from CHANGELOG.md.

Each CHANGELOG bullet looks like `- **Title (`src/x.rs`):** long explanation…`; the release body
keeps only the title so the in-app update dialog (which lists `- ` bullets) stays readable.

Usage: release_notes.py <version> [CHANGELOG.md] [repo e.g. owner/name] > notes.md
"""
import re
import sys

HEADINGS = {
    "security": "Bảo mật",
    "added": "Tính năng mới",
    "changed": "Thay đổi",
    "fixed": "Sửa lỗi",
    "removed": "Đã bỏ",
}


def section(text: str, version: str) -> list[str]:
    lines = text.splitlines()
    start = next(
        (i for i, l in enumerate(lines) if l.startswith(f"## [{version}]")), None
    )
    if start is None:
        return []
    end = next(
        (i for i in range(start + 1, len(lines)) if lines[i].startswith("## [")),
        len(lines),
    )
    return lines[start + 1 : end]


def short_bullet(body: str) -> str:
    m = re.match(r"\*\*(.+?)\*\*", body)
    title = m.group(1) if m else body
    # Drop file references like "(`src/chat.rs`, `src/gui/x.rs`)" and a trailing colon.
    title = re.sub(r"\s*\((?:`[^`]*`(?:,\s*)?)+\)", "", title)
    title = title.strip().rstrip(":").strip()
    if not m and len(title) > 110:
        title = title[:107].rstrip() + "…"
    return title


def build(version: str, changelog: str, repo: str) -> str:
    out = [f"## ctdozlite-next v{version}", ""]
    for line in section(changelog, version):
        s = line.strip()
        if s.startswith("### "):
            key = s[4:].split(" ")[0].lower()
            out += ["", f"### {HEADINGS.get(key, s[4:])}"]
        elif s.startswith("- "):
            title = short_bullet(s[2:])
            if title:
                out.append(f"- {title}")
    if len(out) <= 2:
        out.append("Xem chi tiết trong CHANGELOG.md.")
    base = f"https://github.com/{repo}" if repo else ""
    out += [
        "",
        # Plain line (not a bullet) so the in-app update dialog does not list it.
        f"Chi tiết: [CHANGELOG.md]({base}/blob/v{version}/CHANGELOG.md) · "
        f"Cài đặt: [docs/CAI_DAT.md]({base}/blob/main/docs/CAI_DAT.md). "
        "macOS: chọn `-macos-arm64.dmg` (Apple Silicon), `-macos-x86_64.dmg` (Intel) "
        "hoặc `-macos.dmg` (Universal).",
        "",
        "**Cài trên macOS không bị chặn** (dán vào Terminal):",
        "",
        "```bash",
        f"curl -fsSL https://raw.githubusercontent.com/{repo or 'minhkhang1008/ctdozlite-next'}"
        "/main/scripts/install-macos.sh | bash",
        "```",
    ]
    return "\n".join(out).replace("\n\n\n", "\n\n") + "\n"


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    ver = sys.argv[1].lstrip("v")
    path = sys.argv[2] if len(sys.argv) > 2 else "CHANGELOG.md"
    repo = sys.argv[3] if len(sys.argv) > 3 else ""
    with open(path, encoding="utf-8") as f:
        sys.stdout.write(build(ver, f.read(), repo))
