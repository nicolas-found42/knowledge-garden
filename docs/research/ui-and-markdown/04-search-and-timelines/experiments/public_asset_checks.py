"""Bounded original-file container/text checks, separate from model source cards."""
from pathlib import Path
import json,hashlib,csv,wave,zipfile,xml.etree.ElementTree as ET,subprocess,time
from pypdf import PdfReader
from PIL import Image
R=Path(__file__).resolve().parent;A=R/'assets';results=[]
def check(name,fn):
    st=time.perf_counter()
    try:details=fn();ok=True
    except Exception as e:details={'errorType':type(e).__name__,'message':str(e)};ok=False
    results.append(dict(file=name,passed=ok,seconds=time.perf_counter()-st,sha256=hashlib.sha256((A/name).read_bytes()).hexdigest(),details=details))
def pdf(name,phrases):
    pages=PdfReader(A/name).pages;text='\n'.join(p.extract_text() or '' for p in pages);assert all(s in text for s in phrases);return dict(pages=len(pages),extract=text,checkedPhrases=phrases)
def slides():
    with zipfile.ZipFile(A/'briefing.pptx') as z:
        names=sorted(n for n in z.namelist() if n.startswith('ppt/slides/slide') and n.endswith('.xml') and '/_rels/' not in n)
        extracts=[' '.join(e.text for e in ET.fromstring(z.read(n)).iter() if e.tag.endswith('}t') and e.text) for n in names]
    assert len(extracts)==3 and '12 visits in ten minutes' in extracts[1];return dict(slides=extracts)
def table():
    rows=list(csv.DictReader((A/'visits.csv').open()));assert len(rows)==2 and int(rows[0]['count'])/int(rows[0]['minutes'])==1.2;return dict(rows=rows,rate=1.2)
def image(name):
    with Image.open(A/name) as im:size=im.size;fmt=im.format;im.verify()
    return dict(dimensions=size,format=fmt,semanticFidelity='not checked by container decode')
def tone():
    with wave.open(str(A/'tone.wav')) as w:rate=w.getframerate();frames=w.getnframes()
    assert frames/rate==1;return dict(sampleRate=rate,frames=frames,duration=frames/rate,transcription='not tested; controlled tone contains no speech')
def video():
    r=subprocess.run(['ffprobe','-v','error','-show_entries','format=duration:stream=codec_type,codec_name,width,height','-of','json',str(A/'controlled-demo.mp4')],capture_output=True,text=True,check=True)
    return json.loads(r.stdout)
check('homework.pdf',lambda:pdf('homework.pdf',['1.2 visits per minute','does not estimate all-day activity']))
check('field-report.pdf',lambda:pdf('field-report.pdf',['12 pollinator visits','does not establish a trend']))
check('briefing.pptx',slides);check('visits.csv',table)
for name in ['fruits.jpg','book-pdf-page-84.png']:check(name,lambda n=name:image(n))
check('tone.wav',tone);check('controlled-demo.mp4',video)
(R/'public-asset-results.json').write_text(json.dumps({'checks':results,'passed':sum(x['passed'] for x in results),'total':len(results),'boundary':'8 actual retained files; fixture phrase assertions/container validity only. Model calls use public authored corpus cards, not fresh multimodal extraction. No arbitrary format support or PDF/PPTX visual fidelity claim.'},indent=2)+'\n')
print('public asset checks',sum(x['passed'] for x in results),'/',len(results))
