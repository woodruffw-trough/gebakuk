#!/usr/bin/env python3
"""Rebuild test PDFs with Typst 0.15.1, Pillow 12.3.0 and pypdf 6.10.0.

Python generates fixture assets and PDF edge cases only; it is not an
application dependency. No native PDF renderer is used.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from PIL import GifImagePlugin, Image, ImageDraw
from pypdf import PdfReader, PdfWriter, Transformation
from pypdf.generic import ArrayObject, DictionaryObject, FloatObject, NameObject

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures"
ASSETS = FIXTURES / "assets"
TYPST_VERSION = "0.15.1"


def make_assets():
    ASSETS.mkdir(parents=True, exist_ok=True)
    palette = [244, 248, 251, 14, 116, 144, 237, 128, 55, 21, 42, 64]
    palette += [0] * (768 - len(palette))
    for name in ["orbit", "pulse", "transparent"]:
        frames = []
        for i in range(12):
            frame = Image.new("P", (320, 180), 0)
            frame.putpalette(palette)
            draw = ImageDraw.Draw(frame)
            if name == "orbit":
                draw.rounded_rectangle((6, 6, 313, 173), 16, outline=3, width=3)
                draw.line((40, 90, 280, 90), fill=1, width=3)
                x = 40 + round(240 * (i if i <= 6 else 12 - i) / 6)
                draw.ellipse((x - 18, 72, x + 18, 108), fill=2)
            elif name == "pulse":
                for column in range(7):
                    height = 20 + ((column * 3 + i) % 12) * 9
                    x = 31 + column * 38
                    draw.rounded_rectangle((x, 156 - height, x + 24, 156), 5,
                                           fill=1 if column % 2 else 2)
            else:
                x = 28 + i * 22
                draw.ellipse((x - 16, 74, x + 16, 106), fill=2)
            frames.append(frame)
        options = dict(save_all=True, append_images=frames[1:], duration=120,
                       loop=0, disposal=2, optimize=False)
        if name == "transparent":
            options["transparency"] = 0
        frames[0].save(ASSETS / f"{name}.gif", **options)
        if name == "transparent":
            poster = frames[0].convert("RGBA")
            alpha = frames[0].point(lambda value: 0 if value == 0 else 255, mode="L")
            poster.putalpha(alpha)
        else:
            poster = frames[0].convert("RGB")
        poster.save(ASSETS / f"{name}.png", optimize=False)


def make_transparency_assets():
    """Encode explicit partial frames and local palettes without optimization.

    The first orange circle moves away from (32, 90), exposing PDF artwork.
    The previous-disposal sequence temporarily covers a retained cyan circle
    with magenta, then restores cyan. Clearing instead of restoring fails it.
    """
    orange, cyan, magenta, purple = (
        (237, 128, 55), (28, 218, 230), (218, 63, 150), (116, 82, 230))
    first = (orange, (320, 180), (16, 74, 48, 106), (0, 0), 2)
    partial = (48, 48), (8, 8, 40, 40)
    cases = [
        ("transparent-background", [
            first, (cyan, *partial, (136, 66), 2),
            (magenta, *partial, (256, 66), 2)]),
        ("transparent-previous", [
            first, (cyan, *partial, (136, 66), 1),
            (magenta, *partial, (136, 66), 3),
            (purple, (320, 180), (264, 74, 296, 106), (0, 0), 2)]),
    ]
    for name, specifications in cases:
        frames = []
        for color, size, bounds, _offset, _disposal in specifications:
            frame = Image.new("P", size, 0)
            frame.putpalette([0, 0, 0, *color] + [0] * (768 - 6))
            ImageDraw.Draw(frame).ellipse(bounds, fill=1)
            frame.info["transparency"] = 0
            frames.append(frame)
        header, _ = GifImagePlugin.getheader(
            frames[0], info={"loop": 0, "background": 0, "optimize": False})
        data = b"".join(header)
        for index, frame in enumerate(frames):
            _color, _size, _bounds, offset, disposal = specifications[index]
            data += b"".join(GifImagePlugin.getdata(
                frame, offset=offset, duration=700, transparency=0,
                disposal=disposal, include_color_table=index > 0))
        (ASSETS / f"{name}.gif").write_bytes(data + b";")
        # Pillow composes the encoded frames, so these assertions check the
        # original first-frame position is cleared for both disposal sequences.
        with Image.open(ASSETS / f"{name}.gif") as encoded:
            assert encoded.size == (320, 180) and encoded.n_frames == len(frames)
            encoded.seek(0)
            assert encoded.convert("RGBA").getpixel((32, 90)) == (*orange, 255)
            for index, (color, _size, bounds, offset, _disposal) in enumerate(
                    specifications[1:], 1):
                encoded.seek(index)
                rgba = encoded.convert("RGBA")
                center = (offset[0] + (bounds[0] + bounds[2]) // 2,
                          offset[1] + (bounds[1] + bounds[3]) // 2)
                assert rgba.getpixel((32, 90))[3] == 0
                assert rgba.getpixel(center) == (*color, 255)
            if name == "transparent-previous":
                assert rgba.getpixel((160, 90)) == (*cyan, 255)


def save_writer(writer, name):
    with (FIXTURES / name).open("wb") as output:
        writer.write(output)


def clone(name):
    writer = PdfWriter()
    writer.clone_document_from_reader(PdfReader(FIXTURES / name))
    return writer


def derive_edge_cases():
    # Translate the page content and both page boxes together, preserving the
    # appearance while giving the effective page a nonzero PDF origin.
    writer = clone("plain.pdf")
    for page in writer.pages:
        page.add_transformation(Transformation().translate(40, 25))
        box = ArrayObject([FloatObject(x) for x in (40, 25, 680, 385)])
        page[NameObject("/MediaBox")] = box
        page[NameObject("/CropBox")] = box
    save_writer(writer, "nonzero-origin.pdf")

    writer = clone("plain.pdf")
    for page in writer.pages:
        page.cropbox.lower_left = (40, 25)
        page.cropbox.upper_right = (600, 335)
    save_writer(writer, "cropbox.pdf")

    writer = clone("plain.pdf")
    # One landscape page and one quarter-turn page exercise mixed rotation.
    writer.pages[1].rotate(90)
    save_writer(writer, "rotated.pdf")

    writer = clone("single-gif.pdf")
    writer.pages[0].rotate(90)
    save_writer(writer, "rotated-gif.pdf")

    writer = clone("plain.pdf")
    writer.pages[1].cropbox.upper_right = (600, 300)
    save_writer(writer, "inconsistent-pages.pdf")

    writer = clone("single-gif.pdf")
    tree = writer._root_object["/Names"]["/EmbeddedFiles"]
    child = DictionaryObject({NameObject("/Names"): tree["/Names"]})
    tree.clear()
    tree[NameObject("/Kids")] = ArrayObject([writer._add_object(child)])
    save_writer(writer, "nested-name-tree.pdf")

    writer = clone("single-gif.pdf")
    names = writer._root_object["/Names"]["/EmbeddedFiles"]["/Names"]
    names.extend(names[:2])
    save_writer(writer, "duplicate-identifiers.pdf")


def inspect_fixtures():
    manifest = {"typst_version": TYPST_VERSION, "fixtures": {}}
    for path in sorted(FIXTURES.glob("*.pdf")):
        reader = PdfReader(path)
        attachments = {}
        for name, values in reader.attachments.items():
            for data in values:
                original = ASSETS / name
                if original.exists():
                    assert data == original.read_bytes(), f"Altered bytes: {path}, {name}"
                attachments[name] = {"bytes": len(data),
                                     "sha256": hashlib.sha256(data).hexdigest()}
        pages = []
        for page in reader.pages:
            markers = []
            for reference in page.get("/Annots", []):
                annotation = reference.get_object()
                uri = annotation.get("/A", {}).get("/URI", "")
                if uri.startswith("https://presenter.invalid/gif/"):
                    markers.append({"uri": uri,
                                    "rect": [float(n) for n in annotation["/Rect"]]})
            pages.append({"crop_box": [float(n) for n in page.cropbox],
                          "rotation": page.get("/Rotate", 0), "markers": markers})
        manifest["fixtures"][path.name] = {
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "bytes": path.stat().st_size, "attachments": attachments, "pages": pages}
    (FIXTURES / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Exact attachment names, original data and the predictable annotation
    # rectangle are a guard against changes in Typst's export convention.
    single = manifest["fixtures"]["single-gif.pdf"]
    assert set(single["attachments"]) == {"orbit.gif"}, single["attachments"]
    assert single["pages"][0]["markers"][0]["rect"] == [40, 120, 360, 300]
    print("Verified Typst attachment name orbit.gif and byte-for-byte original GIF data.")
    print("Verified single-GIF PDF rectangle [40, 120, 360, 300] on 640 × 360 pt page.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--typst", default="typst", help="Typst 0.15.1 executable")
    args = parser.parse_args()
    version = subprocess.check_output([args.typst, "--version"], text=True)
    if not version.startswith(f"typst {TYPST_VERSION} "):
        raise SystemExit(f"Expected Typst {TYPST_VERSION}, got {version.strip()}")
    make_assets()
    make_transparency_assets()
    for source in sorted((ROOT / "typst").glob("*.typ")):
        if source.stem in {"theme", "presenter-gif"}:
            continue
        subprocess.run([args.typst, "compile", "--root", str(ROOT),
                        "--ignore-system-fonts", "--creation-timestamp", "0",
                        str(source), str(FIXTURES / f"{source.stem}.pdf")], check=True)
    derive_edge_cases()
    inspect_fixtures()


if __name__ == "__main__":
    main()
