"""Derive public media, exact anchors, and clearly controlled demonstration inputs."""
import json,subprocess,hashlib,urllib.request
from pathlib import Path
import pymupdf
from PIL import Image,ImageChops,ImageDraw,ImageStat
ROOT=Path(__file__).parent
CACHE=Path('/tmp/knowledge-garden-media-lab')
records=[]
for n in [83,84,92]:
 doc=pymupdf.open(CACHE/'book.pdf');page=doc[n-1];name=f'book-pdf-page-{n}.png'
 page.get_pixmap(matrix=pymupdf.Matrix(1.5,1.5)).save(ROOT/name)
 blocks=[dict(text=b[4],rect_points=list(b[:4])) for b in page.get_text('blocks')]
 (ROOT/f'book-pdf-page-{n}-extract.json').write_text(json.dumps(dict(pdf_page_1based=n,printed_page=n-6,page_size_points=list(page.rect),blocks=blocks),indent=2))
 records.append(dict(id=name,parent='book.pdf',anchor=dict(pdf_page_1based=n,printed_page=n-6),method='PyMuPDF render 1.5x'))
for t in [0,2,5,10,20,30]:
 name=f'video-t{t:02d}.png';subprocess.run(['ffmpeg','-y','-loglevel','error','-ss',str(t),'-i',str(CACHE/'vtest.avi'),'-frames:v','1',str(ROOT/name)],check=True)
 records.append(dict(id=name,parent='vtest.avi',anchor=dict(seconds=t,frame_selection='ffmpeg seek nearest decode'),method='ffmpeg still'))
# Deterministic contact sheet: model sees timestamps printed by our script, not source content.
ims=[Image.open(ROOT/f'video-t{t:02d}.png').convert('RGB') for t in [0,2,5,10,20,30]]
sheet=Image.new('RGB',(768*2,576*3),'black');d=ImageDraw.Draw(sheet)
for i,(im,t) in enumerate(zip(ims,[0,2,5,10,20,30])):
 x=(i%2)*768;y=(i//2)*576;sheet.paste(im,(x,y));d.rectangle((x,y,x+110,y+25),fill='black');d.text((x+3,y+3),f'SOURCE t={t}s',fill='white')
sheet.save(ROOT/'video-contact-sheet.jpg',quality=85)
thread=[dict(turn='c1',role='user',text='Controlled research demonstration: use the public OpenCV surveillance sample to find frame changes. Mark changed pixels, without interpreting people or their intent.'),dict(turn='c2',role='assistant',text='First attempt compared RGB values directly with threshold 10, producing noise. I propose blur plus grayscale and threshold 35.'),dict(turn='c3',role='user',text='Use grayscale, a small blur, and threshold 35. Keep frame time anchors. This is an experiment; do not claim the detector identifies suspicious behavior.'),dict(turn='c4',role='assistant',text='Changed controlled_demo.py accordingly and rendered a demonstration. Difference masks are evidence of pixel change, not a reliable object tracker.')]
(ROOT/'controlled-coding-thread.json').write_text(json.dumps(thread,indent=2))
(ROOT/'controlled-personal-note.md').write_text('CONTROLLED PERSONAL NOTE — authored for this experiment, not recovered user history.\n\nI once tried retaining only the fastest benchmark configuration in a toy search project. That lost alternatives useful under changed workloads. I want to revisit whether keeping diverse candidates improves adaptation. No relationship to Darwin or DEAP has been recorded in this note.\n')
for r in records:
 b=(ROOT/r['id']).read_bytes();r.update(sha256=hashlib.sha256(b).hexdigest(),bytes=len(b))
(ROOT/'derivatives.json').write_text(json.dumps(records,indent=2))
