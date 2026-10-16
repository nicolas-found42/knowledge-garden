"""Independently authored review regressions; labels precede extraction runs."""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont
from reportlab.lib.utils import ImageReader
from reportlab.pdfgen import canvas

ROOT = Path(__file__).resolve().parent
FONT = "/System/Library/Fonts/Supplemental/Arial.ttf"
font = ImageFont.truetype(FONT, 46)

pdf = canvas.Canvas(str(ROOT / "pale.pdf"), pagesize=(612, 792))
pdf.setFont("Helvetica", 13)
pdf.drawString(48, 730, "V17 recorded 15 visits.")
image = Image.new("RGB", (1070, 240), "white")
ImageDraw.Draw(image).text(
    (15, 10), "Unrecoverable qualification: duration not verified.",
    font=font, fill=(240, 240, 240),
)
image = image.resize((36, 8)).resize((1070, 240))
pdf.drawImage(ImageReader(image), 80, 300, width=440, height=100)
pdf.save()

pdf = canvas.Canvas(str(ROOT / "inline-raster.pdf"), pagesize=(612, 792))
pdf.setFont("Helvetica", 13)
pdf.drawString(48, 730, "V17 recorded")
pdf.drawString(207, 730, "at Riverside.")
image = Image.new("RGB", (270, 62), "white")
ImageDraw.Draw(image).text(
    (0, -10), "15 visits", font=ImageFont.truetype(FONT, 52), fill="black",
)
pdf.drawImage(ImageReader(image), 132, 729, width=68, height=12)
pdf.save()

pdf = canvas.Canvas(str(ROOT / "repeated-prefix.pdf"), pagesize=(612, 792))
pdf.setFont("Helvetica", 13)
pdf.drawString(48, 730, "The project has 15 visits.")
pdf.drawString(48, 690, "The project has 10 minutes.")
pdf.save()
