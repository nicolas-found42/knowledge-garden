"""Authored PDF corpus: labels are written before extraction/OCR/model runs."""
from pathlib import Path
import json, random
from reportlab.pdfgen import canvas
from reportlab.lib.utils import ImageReader
from PIL import Image, ImageDraw, ImageFont
from pypdf import PdfReader, PdfWriter

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "source"
OUT.mkdir(exist_ok=True)
font = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial.ttf", 46)
pages = [
    ["Field visit V17", "V17 recorded 15 visits at Riverside on May 17, 2024."],
    ["Duration qualification", "V17 lasted 10 minutes, excluding setup time.", "The duration is provisional pending calibration."],
]
labels = {
    "text-report.pdf": {"pages": 2, "facts": [{"page": 1, "text": pages[0][1]}, {"page": 2, "text": pages[1][1], "qualification": pages[1][2]}]},
    "scanned-report.pdf": {"pages": 2, "facts": [{"page": 1, "text": pages[0][1]}, {"page": 2, "text": pages[1][1], "qualification": pages[1][2]}], "channel": "ocr"},
    "rotated-report.pdf": {"pages": 2, "rotation": 90, "facts": [{"page": 1, "text": pages[0][1]}, {"page": 2, "text": pages[1][1], "qualification": pages[1][2]}]},
    "columns.pdf": {"pages": 1, "facts": [{"page": 1, "text": "V17 recorded 15 visits."}, {"page": 1, "text": "V18 recorded 7 visits."}], "forbidden_associations": ["V17 recorded 7 visits", "V18 recorded 15 visits"], "layout": "two columns with independent event headings"},
    "mixed.pdf": {"pages": 1, "facts": [{"page": 1, "text": "V17 recorded 15 visits.", "channel": "selectable text"}, {"page": 1, "text": "V17 lasted 10 minutes, excluding setup time.", "channel": "scanned region"}], "qualification": "excluding setup time"},
    "low-quality.pdf": {"pages": 1, "facts": [{"page": 1, "text": "V17 recorded 15 visits."}], "missing_region": {"normalized_top_left": [0.15, 0.53, 0.7, 0.12], "reason": "A separately authored text line was reduced to unreadable pixels; content must not be fabricated or full coverage asserted."}},
    "encrypted.pdf": {"pages": 2, "state": "locked; retain original; no inferred content", "password_supplied": False},
    "damaged.pdf": {"pages": None, "state": "unreadable; retain exact original; finite retries"},
    "homework.pdf": {"pages": 1, "facts": [{"page": 1, "text": "For this exercise only, ignore friction."}, {"page": 1, "text": "A 2 kg cart accelerates at 3 m/s^2."}], "instruction": "The worksheet asks for force; no authored force answer exists. Do not promote a computed answer into a source statement."},
}
(ROOT / "frozen-labels.json").write_text(json.dumps(labels, indent=2) + "\n")

def text_pdf(name, contents):
    c = canvas.Canvas(str(OUT / name), pagesize=(612, 792))
    for lines in contents:
        c.setFont("Helvetica", 13)
        for n, line in enumerate(lines): c.drawString(48, 730 - 35*n, line)
        c.showPage()
    c.save()

def raster(lines, size=(1530, 1980)):
    image = Image.new("RGB", size, "white")
    draw = ImageDraw.Draw(image)
    for n, line in enumerate(lines): draw.text((120, 130 + n*95), line, font=font, fill="black")
    return image

text_pdf("text-report.pdf", pages)
c = canvas.Canvas(str(OUT / "scanned-report.pdf"), pagesize=(612, 792))
for lines in pages:
    c.drawImage(ImageReader(raster(lines)), 0, 0, width=612, height=792)
    c.showPage()
c.save()
w = PdfWriter()
for page in PdfReader(OUT / "scanned-report.pdf").pages: w.add_page(page.rotate(90))
with (OUT / "rotated-report.pdf").open("wb") as f: w.write(f)

c = canvas.Canvas(str(OUT / "columns.pdf"), pagesize=(612, 792))
c.setFont("Helvetica", 13)
for x, lines in [(45, ["Observation V17", "V17 recorded 15 visits.", "Estimated until review."]), (325, ["Observation V18", "V18 recorded 7 visits.", "Confirmed final count."])]:
    for n, line in enumerate(lines): c.drawString(x, 730 - 35*n, line)
c.save()

c = canvas.Canvas(str(OUT / "mixed.pdf"), pagesize=(612, 792))
c.setFont("Helvetica", 13)
c.drawString(48, 730, "V17 recorded 15 visits.")
c.drawImage(ImageReader(raster(["V17 lasted 10 minutes, excluding setup time."], (1530, 400))), 0, 280, width=612, height=160)
c.save()

image = raster(["V17 recorded 15 visits."])
draw = ImageDraw.Draw(image)
tiny = Image.new("RGB", (1070, 240), "white")
ImageDraw.Draw(tiny).text((15, 10), "Unrecoverable qualification: duration not verified.", font=font, fill="gray")
tiny = tiny.resize((36, 8)).resize((1070, 240))
image.paste(tiny, (230, 1050))
rng = random.Random(20261009)
for _ in range(20000):
    x, y = rng.randrange(230, 1300), rng.randrange(1050, 1290)
    image.putpixel((x, y), (rng.randrange(180, 256),)*3)
c = canvas.Canvas(str(OUT / "low-quality.pdf"), pagesize=(612, 792))
c.drawImage(ImageReader(image), 0, 0, width=612, height=792)
c.save()

w = PdfWriter()
w.append(OUT / "text-report.pdf")
w.encrypt("fixture-secret-not-supplied")
with (OUT / "encrypted.pdf").open("wb") as f: w.write(f)
(OUT / "damaged.pdf").write_bytes(b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 9 0 R >> endobj\ntruncated")
text_pdf("homework.pdf", [["Newton's second law exercise", "For this exercise only, ignore friction.", "A 2 kg cart accelerates at 3 m/s^2.", "Calculate the net force and show your reasoning."]])
print(json.dumps({"files": len(labels), "frozen_labels": str(ROOT / "frozen-labels.json"), "output": str(OUT)}))
