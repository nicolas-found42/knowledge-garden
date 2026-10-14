"""Actual controlled change and runnable public-video difference demonstration.
No object identity or intent inference. Output overlays pixel changes.
"""
from pathlib import Path
import subprocess,json
from PIL import Image,ImageChops,ImageFilter,ImageDraw
ROOT=Path(__file__).parent
CACHE=Path('/tmp/knowledge-garden-media-lab')
THRESHOLD=35
def mask(previous,current):
 a=previous.convert('L').filter(ImageFilter.GaussianBlur(1))
 b=current.convert('L').filter(ImageFilter.GaussianBlur(1))
 return ImageChops.difference(a,b).point(lambda x:255 if x>THRESHOLD else 0)
def main():
 frames=CACHE/'demo-frames';frames.mkdir(exist_ok=True)
 subprocess.run(['ffmpeg','-y','-loglevel','error','-i',str(CACHE/'vtest.avi'),'-t','10','-vf','fps=5',str(frames/'input-%03d.png')],check=True)
 paths=sorted(frames.glob('input-*.png'));previous=Image.open(paths[0]);rows=[]
 for i,path in enumerate(paths):
  current=Image.open(path).convert('RGB');m=mask(previous,current)
  overlay=Image.new('RGB',current.size,(255,0,0));out=Image.composite(overlay,current,m)
  d=ImageDraw.Draw(out);d.rectangle((0,0,768,24),fill='black');d.text((5,5),f'Controlled demo | SOURCE t={i/5:.1f}s | changed pixels, no intent inference',fill='white')
  out.save(frames/f'output-{i:03d}.png');rows.append(dict(seconds=i/5,changed_pixels=sum(v>0 for v in m.getdata()),total_pixels=m.width*m.height));previous=current
 subprocess.run(['ffmpeg','-y','-loglevel','error','-framerate','5','-i',str(frames/'output-%03d.png'),'-c:v','libx264','-pix_fmt','yuv420p',str(ROOT/'controlled-demo.mp4')],check=True)
 Image.open(frames/'output-010.png').save(ROOT/'controlled-demo-screenshot.png')
 (ROOT/'controlled-demo-metrics.json').write_text(json.dumps(rows,indent=2))
if __name__=='__main__':main()
