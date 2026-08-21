#!/usr/bin/env py
"""Build a 16-column hex dump (one byte per column, hex over ASCII)."""

from __future__ import annotations

import html
import struct
import zlib
from pathlib import Path

PNG = Path(__file__).resolve().parent / "sample-with-metadata.png"
OUT_TXT = Path(__file__).resolve().parent / "hexdump.txt"
OUT_HTML = Path(__file__).resolve().parent / "hexdump.html"


def parse_idat(data: bytes) -> tuple[int, int, bytes]:
    i = 8  # after signature
    start = end = None
    parts: list[bytes] = []
    while i + 8 <= len(data):
        ln = struct.unpack(">I", data[i : i + 4])[0]
        typ = data[i + 4 : i + 8]
        cs = i + 8
        ce = cs + ln
        if typ == b"IDAT":
            if start is None:
                start = cs
            end = ce
            parts.append(data[cs:ce])
        if typ == b"IEND":
            break
        i = ce + 4
    assert start is not None and end is not None
    return start, end, b"".join(parts)


def byte_cell(b: int) -> str:
    c = chr(b) if 32 <= b < 127 else "."
    return f"{b:02x}<br>{html.escape(c)}"


def build_rows(data: bytes) -> list[tuple[int, list[int | None]]]:
    rows: list[tuple[int, list[int | None]]] = []
    for row_num, start in enumerate(range(0, len(data), 16), start=1):
        chunk = list(data[start : start + 16])
        cells: list[int | None] = chunk + [None] * (16 - len(chunk))
        rows.append((row_num, cells))
    return rows


def render_html(data: bytes, idat_start: int, idat_end: int, raw: bytes) -> str:
    idat_rows = set(range(idat_start // 16 + 1, (idat_end - 1) // 16 + 2))
    parts = [
        "<!DOCTYPE html><html><head><meta charset='utf-8'>",
        "<title>PNG hex dump</title>",
        "<style>",
        "body{font-family:Consolas,monospace;font-size:12px;margin:16px;}",
        "table{border-collapse:collapse;}",
        "th,td{border:1px solid #ccc;padding:2px 4px;text-align:center;vertical-align:top;}",
        "th{background:#eee;}",
        "td.rownum{background:#f5f5f5;font-weight:bold;}",
        "td.byte{min-width:2.2em;line-height:1.1;}",
        "td.pixel{background:#dfefff;}",
        "td.meta{background:#fff3df;}",
        "td.idat{background:#dfffe0;}",
        "h2{margin-top:24px;}",
        "</style></head><body>",
        f"<h1>{html.escape(PNG.name)}</h1>",
        f"<p>{len(data)} bytes. Each cell = one byte: hex on top, ASCII below.</p>",
        "<table><thead><tr><th>Row</th>",
    ]
    parts.extend(f"<th>{n:02d}</th>" for n in range(1, 17))
    parts.append("</tr></thead><tbody>")

    for row_num, cells in build_rows(data):
        file_off = (row_num - 1) * 16
        parts.append(f"<tr><td class='rownum'>{row_num}</td>")
        for col, b in enumerate(cells, start=1):
            off = file_off + col - 1
            if b is None:
                parts.append("<td></td>")
                continue
            cls = "byte"
            if idat_start <= off < idat_end:
                cls += " idat"
            elif row_num <= 2:
                cls += " meta"
            parts.append(f"<td class='{cls}'>{byte_cell(b)}</td>")
        parts.append("</tr>")

    parts.extend(
        [
            "</tbody></table>",
            "<h2>Where are the 16 pixels?</h2>",
            "<p><strong>Not in rows 1–69.</strong> Those are PNG signature, IHDR, metadata chunks "
            "(iTXt XMP, tEXt, eXIf), and chunk headers/CRCs.</p>",
            f"<p><strong>Compressed pixel data (IDAT):</strong> file bytes {idat_start}–{idat_end - 1}, "
            f"highlighted green above — <strong>rows 70–71</strong> (plus the first byte of row 72).</p>",
            "<ul>",
            "<li>Row 70 cols 05–08: IDAT chunk length (<code>00 00 00 0f</code> = 15 bytes)</li>",
            "<li>Row 70 cols 09–12: chunk type <code>IDAT</code></li>",
            "<li>Row 70 cols 13–16 + row 71 cols 01–16 + row 72 col 01: zlib/DEFLATE payload (15 bytes)</li>",
            "</ul>",
            "<p>The image is <strong>4×4 = 16 pixels</strong>, but PNG never stores them as 16 raw bytes in the file. "
            "After zlib decompression you get 4 scanlines × (1 filter byte + 12 RGB bytes) = 52 bytes of pixel data.</p>",
            "<h2>Decompressed scanlines (actual RGB pixels)</h2>",
            "<table><thead><tr><th>Scanline</th><th>Filter</th>"
            + "".join(f"<th>Pixel {i}</th>" for i in range(1, 5))
            + "</tr></thead><tbody>",
        ]
    )

    # Correct scanline layout: 13 bytes per row if built properly
    stride = 13
    if len(raw) % stride != 0:
        stride = len(raw) // 4
    for r in range(4):
        sl = raw[r * stride : (r + 1) * stride]
        filt = sl[0]
        px = []
        p = 1
        while p + 2 < len(sl):
            px.append(tuple(sl[p : p + 3]))
            p += 3
        while len(px) < 4:
            px.append(("", "", ""))
        cells = "".join(
            f"<td>({a:02x},{b:02x},{c:02x})</td>" if isinstance(a, int) else "<td></td>"
            for a, b, c in px[:4]
        )
        parts.append(f"<tr><td>{r + 1}</td><td>{filt}</td>{cells}</tr>")

    parts.append("</tbody></table></body></html>")
    return "".join(parts)


def render_txt(data: bytes, idat_start: int, idat_end: int) -> str:
    lines = [f"File: {PNG.name} ({len(data)} bytes)\n\n"]
    header = "Row | " + " | ".join(f"{n:02d}" for n in range(1, 17))
    lines.append(header + "\n")
    lines.append("----+-" + "-+-".join(["---"] * 16) + "\n")

    for row_num, cells in build_rows(data):
        hex_cells = []
        asc_cells = []
        for b in cells:
            if b is None:
                hex_cells.append("   ")
                asc_cells.append(" ")
            else:
                hex_cells.append(f"{b:02x}")
                asc_cells.append(chr(b) if 32 <= b < 127 else ".")
        lines.append(f"{row_num:>3} h | " + " | ".join(hex_cells) + "\n")
        lines.append(f"{row_num:>3} a | " + " | ".join(asc_cells) + "\n\n")

    lines.append(
        f"\nIDAT (compressed pixels): bytes {idat_start}-{idat_end - 1} = rows 70-71 (+ row 72 col 01)\n"
    )
    return "".join(lines)


def main() -> None:
    data = PNG.read_bytes()
    idat_start, idat_end, compressed = parse_idat(data)
    raw = zlib.decompress(compressed)
    OUT_TXT.write_text(render_txt(data, idat_start, idat_end), encoding="utf-8")
    OUT_HTML.write_text(render_html(data, idat_start, idat_end, raw), encoding="utf-8")
    print(f"Wrote {OUT_TXT}")
    print(f"Wrote {OUT_HTML}")
    print(f"IDAT: bytes {idat_start}-{idat_end - 1}, rows 70-71, compressed={len(compressed)}, decompressed={len(raw)}")


if __name__ == "__main__":
    main()
