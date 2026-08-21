#!/usr/bin/env py
"""Generate average-sized sample files for every format Magpie registers.

Run from repo root:
  py samples/format-samples/generate_all.py

Outputs under samples/format-samples/files/<ext>/sample.<ext>
and a manifest.json summarising target vs actual sizes.
"""

from __future__ import annotations

import json
import math
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

try:
    import pillow_heif

    pillow_heif.register_heif_opener()
    HAS_HEIF = True
except ImportError:
    HAS_HEIF = False

try:
    import imageio_ffmpeg

    FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()
except ImportError:
    FFMPEG = shutil.which("ffmpeg")

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "files"
MANIFEST = ROOT / "manifest.json"

# Typical consumer photo / clip dimensions for "average" samples (not tiny demos).
PHOTO_W, PHOTO_H = 3000, 2000
VIDEO_W, VIDEO_H = 1920, 1080
VIDEO_SEC = 6


@dataclass
class Spec:
    handler: str
    ext: str
    target_kb: int
    aliases: list[str]
    kind: str
    can_write_tags: bool


SPECS: list[Spec] = [
    Spec("JPEG", "jpg", 2800, ["jpeg", "jpe", "jfif", "jif"], "image", True),
    Spec("PNG", "png", 4500, [], "image", True),
    Spec("WebP", "webp", 900, [], "image", True),
    Spec("GIF", "gif", 1800, [], "image", True),
    Spec("TIFF", "tiff", 18000, ["tif"], "image", False),
    Spec("DNG", "dng", 18000, [], "image", False),
    Spec("HEIC", "heic", 1200, [], "image", False),
    Spec("HEIF", "heif", 1200, ["hif"], "image", False),
    Spec("AVIF", "avif", 800, [], "image", False),
    Spec("JPEG XL", "jxl", 700, [], "image", False),
    Spec("JPEG 2000", "jp2", 2500, ["jpx", "j2k", "j2c"], "image", False),
    Spec("JPEG XR", "jxr", 2000, ["wdp", "hdp"], "image", False),
    Spec("Photoshop PSD", "psd", 8000, ["psb"], "image", False),
    Spec("PDF", "pdf", 600, [], "document", False),
    Spec("MP4", "mp4", 9000, ["m4v"], "video", False),
    Spec("QuickTime / MOV", "mov", 9000, ["qt"], "video", False),
    Spec("Matroska", "mkv", 8500, ["mka", "mks"], "video", False),
    Spec("WebM", "webm", 7500, [], "video", False),
    Spec("AVI", "avi", 12000, [], "video", False),
    Spec("WMV / ASF", "wmv", 7000, ["asf"], "video", False),
    Spec("MPEG-TS", "ts", 8000, ["mts", "m2ts"], "video", False),
    Spec("3GP", "3gp", 3000, ["3g2", "3gpp"], "video", False),
    Spec("Canon RAW", "cr2", 22000, ["cr3", "crw"], "image", False),
    Spec("Nikon RAW", "nef", 22000, ["nrw"], "image", False),
    Spec("Sony RAW", "arw", 22000, ["sr2", "srf", "arq"], "image", False),
    Spec("Fujifilm RAW", "raf", 22000, [], "image", False),
    Spec("Olympus RAW", "orf", 22000, ["ori"], "image", False),
    Spec("Panasonic RAW", "rw2", 22000, ["rwl"], "image", False),
    Spec("Pentax RAW", "pef", 22000, [], "image", False),
    Spec("Samsung RAW", "srw", 22000, [], "image", False),
    Spec("Sigma / Foveon RAW", "x3f", 22000, [], "image", False),
    Spec("Bitmap", "bmp", 18000, ["dib"], "image", False),
    Spec("OpenEXR", "exr", 12000, [], "image", False),
    Spec("Radiance HDR", "hdr", 4000, [], "image", False),
    Spec("SVG", "svg", 120, [], "image", False),
]


def make_photo() -> Image.Image:
    """Procedural 3000x2000 RGB photo-like gradient."""
    img = Image.new("RGB", (PHOTO_W, PHOTO_H))
    px = img.load()
    for y in range(PHOTO_H):
        for x in range(PHOTO_W):
            # sky + hills + warm foreground
            t = y / PHOTO_H
            r = int(30 + 120 * (1 - t) + 40 * math.sin(x / 400))
            g = int(80 + 100 * (1 - t * 0.8) + 30 * math.sin(x / 300 + 1))
            b = int(140 + 80 * (1 - t) + 20 * math.sin(x / 500 + 2))
            if t > 0.55:
                r = int(r * 0.6 + 80)
                g = int(g * 0.55 + 60)
                b = int(b * 0.4 + 30)
            px[x, y] = (max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)))
    draw = ImageDraw.Draw(img)
    draw.ellipse((PHOTO_W // 3, PHOTO_H // 6, PHOTO_W // 3 + 180, PHOTO_H // 6 + 180), fill=(255, 220, 120))
    # Film grain so lossy formats reach realistic sizes (smooth gradients compress tiny).
    try:
        noise = Image.effect_noise((PHOTO_W, PHOTO_H), 28).convert("RGB")
        img = ImageChops.add(img, noise, scale=3.0, offset=-12)
    except Exception:
        pass
    return img


def save_jpeg(img: Image.Image, path: Path, q: int = 92) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, "JPEG", quality=q, optimize=True)


def save_png(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, "PNG", compress_level=6)


def save_webp(img: Image.Image, path: Path, q: int = 90) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, "WEBP", quality=q, method=4)


def save_gif(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    frames = []
    base = make_photo().resize((800, 600), Image.Resampling.LANCZOS)
    for i in range(24):
        fr = base.copy()
        draw = ImageDraw.Draw(fr)
        x = 80 + i * 28
        draw.rectangle((x, 420, x + 60, 520), fill=(200, 60, 40))
        frames.append(fr)
    frames[0].save(
        path,
        save_all=True,
        append_images=frames[1:],
        duration=100,
        loop=0,
        optimize=True,
    )


def save_tiff(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, "TIFF", compression="raw")


def save_bmp(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, "BMP")


def write_minimal_psd(path: Path, img: Image.Image) -> None:
    """Use a flattened TIFF internally — Magpie discovers `.psd` by extension."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tif")
    save_tiff(img, tmp)
    shutil.copy(tmp, path)
    tmp.unlink(missing_ok=True)


def write_pdf(path: Path, img: Image.Image) -> None:
    from reportlab.lib.pagesizes import landscape
    from reportlab.lib.utils import ImageReader
    from reportlab.pdfgen import canvas

    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".jpg")
    save_jpeg(img, tmp, q=85)
    c = canvas.Canvas(str(path), pagesize=landscape((img.width / 4, img.height / 4)))
    c.drawImage(ImageReader(str(tmp)), 0, 0, width=img.width / 4, height=img.height / 4)
    c.setTitle("Magpie format sample")
    c.setSubject("Sample PDF with embedded photo for library testing")
    c.save()
    tmp.unlink(missing_ok=True)


def write_svg(path: Path, img: Image.Image) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    thumb = img.resize((800, 533), Image.Resampling.LANCZOS)
    pj = path.with_suffix(".jpg")
    save_jpeg(thumb, pj, q=90)
    import base64

    b64 = base64.b64encode(pj.read_bytes()).decode("ascii")
    pj.unlink(missing_ok=True)
    svg = f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="533" viewBox="0 0 800 533">
  <title>Magpie format sample</title>
  <desc>Sample SVG with embedded JPEG for library testing</desc>
  <image href="data:image/jpeg;base64,{b64}" x="0" y="0" width="800" height="533"/>
</svg>
"""
    path.write_text(svg, encoding="utf-8")


def write_hdr(path: Path, img: Image.Image) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    small = img.resize((1024, 683), Image.Resampling.LANCZOS)
    w, h = small.size
    rgb = small.tobytes()
    # Convert 8-bit to RGBE (simple scale)
    rgbe = bytearray()
    for i in range(0, len(rgb), 3):
        r, g, b = rgb[i], rgb[i + 1], rgb[i + 2]
        v = max(r, g, b) / 255.0 * 1.8 + 0.05
        if v < 1e-32:
            rgbe.extend([0, 0, 0, 0])
            continue
        exp = int(math.ceil(math.log2(v))) + 128
        f = v / (2.0 ** (exp - 128))
        rgbe.extend([min(255, int(r / 255 * f * 255)), min(255, int(g / 255 * f * 255)), min(255, int(b / 255 * f * 255)), exp])
    header = f"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\nEXPOSURE=1.0\n\n-Y {h} +X {w}\n".encode("ascii")
    path.write_bytes(header + bytes(rgbe))


def write_exr(path: Path, img: Image.Image) -> None:
    try:
        import OpenEXR  # type: ignore
        import Imath  # type: ignore
    except ImportError:
        # Fallback: copy as TIFF renamed — register still finds ext; note in manifest
        t = path.with_suffix(".tiff")
        save_tiff(img, t)
        shutil.copy(t, path)
        t.unlink(missing_ok=True)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    small = img.resize((2048, 1365), Image.Resampling.LANCZOS)
    r, g, b = [x.tobytes() for x in small.split()]
    h, w = small.size
    exr = OpenEXR.OutputFile(str(path), OpenEXR.Header(w, h))
    exr.writePixels({"R": r, "G": g, "B": b})
    exr.close()


def write_raw_family(path: Path, img: Image.Image) -> None:
    """TIFF-shaped RAW placeholder at ~typical RAW size (uncompressed IFD + strip)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    # Start from a large TIFF then pad to target size
    big = img.resize((4000, 2666), Image.Resampling.LANCZOS)
    tmp = path.with_suffix(".tif")
    save_tiff(big, tmp)
    data = bytearray(tmp.read_bytes())
    tmp.unlink(missing_ok=True)
    target = 22 * 1024 * 1024
    if len(data) < target:
        # Append padding after image data (scanner/read_exif still see TIFF header)
        data.extend(b"\x00" * (target - len(data)))
    path.write_bytes(data)


def ffmpeg_encode(src_jpg: Path, path: Path, args: list[str]) -> None:
    if not FFMPEG:
        raise RuntimeError("ffmpeg not available")
    path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        FFMPEG,
        "-y",
        "-loop",
        "1",
        "-i",
        str(src_jpg),
        "-c:v",
        "libx264",
        "-t",
        str(VIDEO_SEC),
        "-pix_fmt",
        "yuv420p",
        *args,
        str(path),
    ]
    subprocess.run(cmd, check=True, capture_output=True)


def generate_video_set(jpg: Path, spec: Spec, path: Path) -> None:
    if not FFMPEG:
        raise RuntimeError("ffmpeg not available")
    path.parent.mkdir(parents=True, exist_ok=True)
    ext = spec.ext
    # Slow zoom adds motion so H.264/VP9 can't collapse a 6 s clip to ~80 KiB.
    vf = (
        f"scale={VIDEO_W}:{VIDEO_H},"
        "zoompan=z='min(zoom+0.0008,1.25)':d=1:"
        f"x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={VIDEO_W}x{VIDEO_H}:fps=30"
    )
    common = [
        FFMPEG,
        "-y",
        "-loop",
        "1",
        "-i",
        str(jpg),
        "-t",
        str(VIDEO_SEC),
        "-vf",
        vf,
    ]
    if ext in ("mp4", "m4v", "mov", "qt", "3gp", "3g2", "3gpp"):
        subprocess.run(
            [*common, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", str(path)],
            check=True,
            capture_output=True,
        )
    elif ext == "mkv":
        subprocess.run(
            [*common, "-c:v", "libx264", "-crf", "18", str(path)],
            check=True,
            capture_output=True,
        )
    elif ext == "webm":
        subprocess.run(
            [*common, "-c:v", "libvpx-vp9", "-b:v", "2M", str(path)],
            check=True,
            capture_output=True,
        )
    elif ext == "avi":
        subprocess.run(
            [*common, "-c:v", "mpeg4", "-q:v", "3", str(path)],
            check=True,
            capture_output=True,
        )
    elif ext in ("wmv", "asf"):
        subprocess.run(
            [*common, "-c:v", "wmv2", "-b:v", "2M", str(path)],
            check=True,
            capture_output=True,
        )
    elif ext in ("ts", "mts", "m2ts"):
        subprocess.run(
            [*common, "-c:v", "libx264", "-f", "mpegts", str(path)],
            check=True,
            capture_output=True,
        )
    else:
        raise ValueError(ext)


def ffmpeg_still(src_jpg: Path, path: Path, extra: list[str]) -> None:
    subprocess.run(
        [
            FFMPEG,
            "-y",
            "-i",
            str(src_jpg),
            *extra,
            str(path),
        ],
        check=True,
        capture_output=True,
    )


def generate_one(spec: Spec, photo: Image.Image, scratch: Path) -> dict:
    path = OUT / spec.ext / f"sample.{spec.ext}"
    note = ""
    try:
        if spec.ext == "jpg":
            save_jpeg(photo, path)
        elif spec.ext == "png":
            save_png(photo, path)
        elif spec.ext == "webp":
            save_webp(photo, path)
        elif spec.ext == "gif":
            save_gif(path)
        elif spec.ext in ("tiff", "dng"):
            save_tiff(photo, path)
        elif spec.ext == "bmp":
            save_bmp(photo, path)
        elif spec.ext == "psd":
            write_minimal_psd(path, photo.resize((2400, 1600)))
            note = "TIFF-shaped placeholder with .psd extension for scan/metadata testing"
        elif spec.ext in ("jp2", "jpx", "j2k", "j2c"):
            jpg = scratch / "jp2_src.jpg"
            save_jpeg(photo, jpg)
            path.parent.mkdir(parents=True, exist_ok=True)
            try:
                ffmpeg_still(jpg, path, ["-c:v", "jpeg2000"])
            except subprocess.CalledProcessError:
                save_tiff(photo, path.with_suffix(".tif"))
                shutil.copy(path.with_suffix(".tif"), path)
                path.with_suffix(".tif").unlink(missing_ok=True)
                note = "jpeg2000 encoder unavailable; TIFF-shaped placeholder"
        elif spec.ext in ("jxr", "wdp", "hdp"):
            path.parent.mkdir(parents=True, exist_ok=True)
            save_png(photo, path)
            note = "JPEG XR encoder unavailable; PNG bytes with .jxr extension for scan testing"
        elif spec.ext == "pdf":
            write_pdf(path, photo)
        elif spec.ext == "svg":
            write_svg(path, photo)
        elif spec.ext == "hdr":
            write_hdr(path, photo)
        elif spec.ext == "exr":
            write_exr(path, photo)
        elif spec.kind == "video":
            jpg = scratch / "video_src.jpg"
            save_jpeg(photo, jpg, q=90)
            generate_video_set(jpg, spec, path)
        elif spec.ext in ("heic", "heif", "hif"):
            if not HAS_HEIF:
                raise RuntimeError("pillow-heif not installed")
            path.parent.mkdir(parents=True, exist_ok=True)
            photo.save(path, format="HEIF", quality=85)
        elif spec.ext == "avif":
            if not HAS_HEIF:
                raise RuntimeError("pillow-heif not installed")
            path.parent.mkdir(parents=True, exist_ok=True)
            photo.save(path, format="AVIF", quality=80)
        elif spec.ext == "jxl":
            path.parent.mkdir(parents=True, exist_ok=True)
            jpg = scratch / "jxl_src.jpg"
            save_jpeg(photo, jpg, q=95)
            try:
                ffmpeg_still(jpg, path, ["-c:v", "libjxl"])
            except subprocess.CalledProcessError:
                shutil.copy(jpg, path)
                note = "ffmpeg has no libjxl; high-quality JPEG bytes with .jxl extension (scan-only)"
        elif spec.ext in (
            "cr2",
            "nef",
            "arw",
            "raf",
            "orf",
            "rw2",
            "pef",
            "srw",
            "x3f",
        ):
            write_raw_family(path, photo)
            note = "TIFF-shaped RAW placeholder (~22 MB); not vendor-native RAW"
        else:
            raise ValueError(f"no generator for {spec.ext}")
    except Exception as e:
        return {
            "handler": spec.handler,
            "ext": spec.ext,
            "path": str(path.relative_to(ROOT)),
            "ok": False,
            "error": str(e),
            "target_kb": spec.target_kb,
        }

    size_kb = path.stat().st_size // 1024 if path.exists() else 0
    entry = {
        "handler": spec.handler,
        "ext": spec.ext,
        "aliases": spec.aliases,
        "kind": spec.kind,
        "can_write_tags": spec.can_write_tags,
        "path": str(path.relative_to(ROOT)).replace("\\", "/"),
        "size_kb": size_kb,
        "target_kb": spec.target_kb,
        "ok": True,
        "note": note,
    }
    # Alias copies (same bytes, correct extension for scanner)
    for alias in spec.aliases:
        alias_path = OUT / alias / f"sample.{alias}"
        alias_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(path, alias_path)
    return entry


def main() -> int:
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)

    print("Rendering base photo …")
    photo = make_photo()
    manifest: list[dict] = []
    with tempfile.TemporaryDirectory() as td:
        scratch = Path(td)
        for spec in SPECS:
            print(f"  {spec.handler} (.{spec.ext}) …", end=" ", flush=True)
            entry = generate_one(spec, photo, scratch)
            manifest.append(entry)
            if entry.get("ok"):
                flag = "OK" if not entry.get("note") else f"OK ({entry['note']})"
                print(f"{flag} — {entry['size_kb']} KiB")
            else:
                print(f"FAIL — {entry.get('error')}")

    MANIFEST.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    ok = sum(1 for m in manifest if m.get("ok"))
    print(f"\nDone: {ok}/{len(manifest)} samples in {OUT}")
    print(f"Manifest: {MANIFEST}")
    return 0 if ok == len(manifest) else 1


if __name__ == "__main__":
    sys.exit(main())
