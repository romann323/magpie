#!/usr/bin/env py
"""Create a minimal PNG with embedded metadata chunks and print its structure."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

OUT = Path(__file__).resolve().parent / "sample-with-metadata.png"

PNG_SIG = b"\x89PNG\r\n\x1a\n"


def crc32(chunk_type: bytes, data: bytes) -> int:
    return zlib.crc32(chunk_type + data) & 0xFFFFFFFF


def make_chunk(chunk_type: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + chunk_type
        + data
        + struct.pack(">I", crc32(chunk_type, data))
    )


def make_ihdr(width: int, height: int) -> bytes:
    # 8-bit RGB, no interlace
    data = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return make_chunk(b"IHDR", data)


def make_idat(width: int, height: int, rgb: tuple[int, int, int]) -> bytes:
    r, g, b = rgb
    row = bytes([0, r, g, b]) * width  # filter byte 0 + pixels
    raw = row * height
    compressed = zlib.compress(raw, 9)
    return make_chunk(b"IDAT", compressed)


def make_itxt_xmp(xmp_xml: str) -> bytes:
    keyword = b"XML:com.adobe.xmp"
    # iTXt: keyword\0 compression_flag compression_method language\0 translated\0 text
    data = (
        keyword
        + b"\x00"  # end keyword
        + b"\x00"  # uncompressed
        + b"\x00"  # compression method (ignored)
        + b"\x00"  # empty language tag
        + b"\x00"  # empty translated keyword
        + xmp_xml.encode("utf-8")
    )
    return make_chunk(b"iTXt", data)


def make_text(keyword: str, text: str) -> bytes:
    data = keyword.encode("latin-1") + b"\x00" + text.encode("latin-1")
    return make_chunk(b"tEXt", data)


def make_exif_chunk() -> bytes:
    # Minimal TIFF/EXIF blob: little-endian, one IFD with ImageDescription.
    # This is illustrative — real camera EXIF is much larger.
    tiff = bytearray()
    tiff += b"II"  # little-endian
    tiff += struct.pack("<H", 42)
    ifd0_offset = 8
    tiff += struct.pack("<I", ifd0_offset)

    # IFD at offset 8: 1 entry + next-IFD pointer
    desc = b"Sample PNG metadata demo"
    desc_offset = 8 + 2 + 12 + 4  # after IFD header
    ifd = struct.pack("<H", 1)  # 1 tag
    # Tag 270 ImageDescription, type ASCII (2), count includes NUL
    ifd += struct.pack("<HHII", 270, 2, len(desc) + 1, desc_offset)
    ifd += struct.pack("<I", 0)  # no next IFD
    tiff += ifd
    tiff += desc + b"\x00"
    return make_chunk(b"eXIf", bytes(tiff))


XMP_PACKET = """<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Magpie demo">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:xmp="http://ns.adobe.com/xap/1.0/">
      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Harbour at dusk</rdf:li>
        </rdf:Alt>
      </dc:title>
      <dc:description>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Demo PNG showing where metadata lives.</rdf:li>
        </rdf:Alt>
      </dc:description>
      <dc:subject>
        <rdf:Bag>
          <rdf:li>demo</rdf:li>
          <rdf:li>png</rdf:li>
          <rdf:li>metadata</rdf:li>
        </rdf:Bag>
      </dc:subject>
      <xmp:Rating>4</xmp:Rating>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"""


def build_png() -> bytes:
    parts = [
        PNG_SIG,
        make_ihdr(4, 4),
        make_itxt_xmp(XMP_PACKET),  # Magpie / Adobe / Explorer convention
        make_text("Author", "Magpie metadata demo"),
        make_text("Software", "create_and_inspect.py"),
        make_exif_chunk(),
        make_idat(4, 4, (70, 130, 180)),  # steel blue pixels
        make_chunk(b"IEND", b""),
    ]
    return b"".join(parts)


def hexdump(data: bytes, max_bytes: int = 64) -> str:
    shown = data[:max_bytes]
    hexpart = " ".join(f"{b:02x}" for b in shown)
    if len(data) > max_bytes:
        hexpart += f" ... (+{len(data) - max_bytes} bytes)"
    return hexpart


def parse_itxt(data: bytes) -> dict[str, str]:
    nul = data.index(0)
    keyword = data[:nul].decode("utf-8", errors="replace")
    comp_flag = data[nul + 1]
    lang_start = nul + 3
    lang_nul = data.index(0, lang_start)
    trans_start = lang_nul + 1
    trans_nul = data.index(0, trans_start)
    text = data[trans_nul + 1 :].decode("utf-8", errors="replace")
    return {
        "keyword": keyword,
        "compressed": str(bool(comp_flag)),
        "text_preview": text[:120].replace("\n", " ") + ("..." if len(text) > 120 else ""),
    }


def parse_text(data: bytes) -> dict[str, str]:
    nul = data.index(0)
    return {
        "keyword": data[:nul].decode("latin-1", errors="replace"),
        "text": data[nul + 1 :].decode("latin-1", errors="replace"),
    }


def inspect_png(path: Path) -> None:
    raw = path.read_bytes()
    print(f"File: {path}")
    print(f"Size: {len(raw)} bytes\n")
    print("PNG signature:", hexdump(raw[:8], 8))
    print()

    i = len(PNG_SIG)
    idx = 0
    while i + 8 <= len(raw):
        length = struct.unpack(">I", raw[i : i + 4])[0]
        ctype = raw[i + 4 : i + 8]
        data_start = i + 8
        data_end = data_start + length
        crc = raw[data_end : data_end + 4]
        data = raw[data_start:data_end]
        idx += 1
        label = ctype.decode("latin-1", errors="replace")
        print(f"Chunk #{idx}: {label}  length={length}  CRC={crc.hex()}")

        if ctype == b"IHDR" and length >= 13:
            w, h, depth, color, comp, filt, inter = struct.unpack(">IIBBBBB", data[:13])
            print(f"  -> {w}x{h} px, bit depth {depth}, color type {color}")

        elif ctype == b"iTXt":
            info = parse_itxt(data)
            print(f"  -> keyword: {info['keyword']!r}, compressed={info['compressed']}")
            if info["keyword"] == "XML:com.adobe.xmp":
                print("  -> XMP payload (standard slot for tags/title/rating):")
                print(f"     {info['text_preview']}")

        elif ctype == b"tEXt":
            info = parse_text(data)
            print(f"  -> tEXt keyword: {info['keyword']!r} = {info['text']!r}")

        elif ctype == b"eXIf":
            print(f"  -> EXIF/TIFF blob, {len(data)} bytes, header: {hexdump(data, 16)}")
            if data[:2] == b"II":
                print("  -> little-endian TIFF wrapper (camera-style metadata)")

        elif ctype == b"IDAT":
            print(f"  -> compressed pixel data, {len(data)} bytes")

        elif ctype == b"IEND":
            print("  -> end of PNG")
            break

        print()
        i = data_end + 4


def main() -> None:
    png = build_png()
    OUT.write_bytes(png)
    inspect_png(OUT)


if __name__ == "__main__":
    main()
