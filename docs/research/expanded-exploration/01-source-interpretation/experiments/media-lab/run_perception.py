"""Purposeful observation vs interpretation probes, public media only.
Raw response retained; request media is referenced by immutable hash, not repeated base64.
"""
import os,json,time,base64,hashlib,urllib.request,urllib.error,concurrent.futures
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).parent;CACHE=Path('/tmp/knowledge-garden-media-lab');OUT=ROOT/'perception-records';OUT.mkdir(exist_ok=True)
MODEL='google/gemini-2.5-flash-lite'
OBS='Return concise JSON with observations:[{claim,anchor,uncertainty}], exact_visible_text, and limitations. Describe only what the supplied image/video directly shows. Separate uncertain interpretations. Anchor image claims by approximate normalized top-left bounding box [x,y,w,h]; video claims by seconds. Never infer author motives, historical links, exact weights, medical facts, identity, or security intent from appearances. Preserve qualifications and negations in book text. Do not normalize familiar phrases into a different sentence.'
INTERP='Return concise JSON with observations and interpretive_candidates:[{claim,anchor,status,why}], limitations. Suggest useful interpretations or research connections, marking hypothesis vs directly observed facts. Do not claim connections were actually used historically, do not infer identities or suspicious intent. Preserve uncertainty and source anchors.'
Image.open(ROOT/'phototest.tif').save(ROOT/'phototest.png')
TASKS=[]
for name in ['phototest.png','book-pdf-page-83.png','book-pdf-page-84.png','book-pdf-page-92.png','fruits.jpg','video-contact-sheet.jpg','controlled-demo-screenshot.png']:
 for mode in ['observation','interpretation']:TASKS.append(dict(id=name.rsplit('.',1)[0]+'-'+mode,file=str(ROOT/name),mode=mode,media='image'))
TASKS.append(dict(id='native-video-observation',file=str(CACHE/'source-10s.mp4'),mode='observation',media='video'))
# A real across-format hypothesis generator, with source facts rather than invented user history.
TASKS.append(dict(id='book-project-connection',file=str(ROOT/'book-pdf-page-84.png'),mode='interpretation',media='image',extra='CONTROLLED PERSONAL NOTE: '+(ROOT/'controlled-personal-note.md').read_text()+'\nPUBLIC DEAP AUTHOR TUTORIAL EXCERPT:\n'+(ROOT/'article-selection.rst').read_text()[4500:6900]+'\nOLD PUBLIC PROJECT IMPLEMENTATION:\n'+(ROOT/'old-project-selection.py').read_text()[:6500]+'\nPropose potential useful links, plus weak or misleading links to avoid. The controlled note was authored now; DEAP is an old public project, not evidence of this user’s actual old project.'))
def call(t):
 path=Path(t['file']);b=path.read_bytes();mime='video/mp4' if t['media']=='video' else ('image/jpeg' if path.suffix=='.jpg' else 'image/png')
 prompt=(OBS if t['mode']=='observation' else INTERP)+'\n'+t.get('extra','')
 content=[{'type':'text','text':prompt},{'type':t['media']+'_url',t['media']+'_url':{'url':f'data:{mime};base64,'+base64.b64encode(b).decode()}}]
 payload=dict(model=MODEL,messages=[dict(role='user',content=content)],temperature=0,max_tokens=1800)
 req=urllib.request.Request('https://openrouter.ai/api/v1/chat/completions',data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+os.environ['OPENROUTER_API_KEY'],'Content-Type':'application/json'})
 record=dict(task=t,model_requested=MODEL,prompt=prompt,media_sha256=hashlib.sha256(b).hexdigest(),media_bytes=len(b));start=time.perf_counter()
 try:
  record['response']=json.loads(urllib.request.urlopen(req,timeout=90).read())
 except urllib.error.HTTPError as e:record.update(error_type=type(e).__name__,status=e.code,error_body=e.read().decode()[:3000])
 except Exception as e:record.update(error_type=type(e).__name__,error=str(e))
 record['seconds']=time.perf_counter()-start;(OUT/(t['id']+'.json')).write_text(json.dumps(record,indent=2));return t['id'],record.get('response',{}).get('usage'),record.get('error_type')
if __name__=='__main__':
 pending=[t for t in TASKS if not (OUT/(t['id']+'.json')).exists()]
 with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
  for r in pool.map(call,pending):print(r,flush=True)
