"""Controlled first attempt: noisy direct RGB difference, authored research history."""
from PIL import ImageChops
THRESHOLD=10
def mask(previous,current):
 return ImageChops.difference(previous.convert('RGB'),current.convert('RGB')).convert('L').point(lambda x:255 if x>THRESHOLD else 0)
