"""Freeze claim labels before judgments; compare representations and cross-source candidates."""
import json,os,time,urllib.request,urllib.error,concurrent.futures,hashlib
from pathlib import Path
ROOT=Path(__file__).parent;OUT=ROOT/'judgment-records';OUT.mkdir(exist_ok=True)
def text_record(name):
 d=json.loads((ROOT/'perception-records'/f'{name}.json').read_text());return d.get('response',{}).get('choices',[{}])[0].get('message',{}).get('content','ERROR: supporting model failed')
def ocr(name):
 d=json.loads((ROOT/f'{name}-ocr.json').read_text());return '\n'.join(x['text'] for x in d) if isinstance(d,list) else str(d)
def extract(n):
 d=json.loads((ROOT/f'book-pdf-page-{n}-extract.json').read_text());return '\n'.join(x['text'] for x in d['blocks'])
CLAIMS={
'phototest': [('dog','The text says a quick brown dog jumped over a lazy fox.','supported'),('cliche','The text says a quick brown fox jumped over a lazy dog.','contradicted'),('font','The text mentions 12 point text.','supported'),('success','The OCR code passed all file format tests.','unknown')],
'book-pdf-page-84':[('selection','Natural Selection preserves favourable variations and rejects injurious variations.','supported'),('neutral','Variations neither useful nor injurious are unaffected by natural selection.','supported'),('all','Every variation is destroyed by natural selection.','contradicted'),('future','The author predicts that all future software should use evolutionary algorithms.','unknown')],
'book-pdf-page-92':[('tradeoff','The swifter wolves have the best chance only provided they retain strength to master prey.','supported'),('speed','The author says speed alone is sufficient regardless of strength to master prey.','contradicted'),('intent','The user used this wolf passage to design their benchmark tool.','unknown')],
'fruits':[('fruit','There are several kinds of fruit, including bananas.','supported'),('weight','The bananas weigh exactly 250 grams.','unknown'),('nutrition','The image establishes the vitamin C content of every fruit.','unknown'),('usage','The fruit photograph was used by this user for crop planning.','unknown')],
'video-contact-sheet':[('movement','People occupy different positions in the supplied timestamped frames.','supported'),('static','All people remain in exactly the same positions in all shown frames.','contradicted'),('intent','One person is committing a theft.','unknown'),('trajectory','The exact uninterrupted trajectory of every person between 0 and 30 seconds is established.','unknown'),('reason','The video establishes why the programmer chose threshold 35.','unknown')],
'controlled-demo-screenshot':[('overlay','The displayed screenshot includes red changed pixel overlays.','supported'),('threshold','The screenshot independently establishes that the threshold equals 35.','unknown'),('crime','Red highlights prove that a suspicious person was detected.','unknown')]
}
CASES=[]
for source,claims in CLAIMS.items():
 if source=='video-contact-sheet':exact='\n'.join(f't={t}s OCR:\n'+ocr(f'video-t{t:02d}') for t in [0,2,5,10,20,30])
 elif source=='controlled-demo-screenshot':exact='CONTROLLED IMAGE LABEL: changed pixels, no intent inference; SOURCE t=2.0s. No other text extracted.'
 else:exact=ocr(source)
 reps={'extractive':exact,'observation':text_record(source+'-observation'),'interpretation':text_record(source+'-interpretation')}
 if source.startswith('book'):reps['pdf-text']=extract(int(source[-2:]))
 for mode,evidence in reps.items():
  for cid,claim,label in claims:
   CASES.append(dict(id=f'{source}-{mode}-{cid}',family='claim',source=source,representation=mode,claim=claim,source_truth=label,state=dict(source=source,evidence_kind=mode,evidence=evidence,claim=claim),questions={'support':dict(type='choice',instructions='Using only the supplied evidence, is the full claim supported, contradicted, or unknown? Preserve negations, qualifiers, causal scope, and limits of temporal sampling. Generated speculative hypotheses do not establish real historical facts. No outside source may be assumed.',criteria={'supported':'All factual clauses established by supplied evidence.','contradicted':'Evidence directly contradicts claim.','unknown':'Missing clause, uncertain perception, hypothetical, or unsupported causal/history assertion.'})}))
# Cross-source connections: labels concern semantic relation, not proof of real user history.
book=extract(84)+'\n'+extract(92);article=(ROOT/'article-selection.rst').read_text()[4500:6900];code=(ROOT/'old-project-selection.py').read_text()[:6500];note=(ROOT/'controlled-personal-note.md').read_text();thread=json.loads((ROOT/'controlled-coding-thread.json').read_text())
RELATIONS=[
 ('adaptation','Darwin’s qualification about environmental change suggests a research analogy for revisiting configurations under changed workloads.','hypothesis'),
 ('history','The user’s old benchmark project was historically inspired by this Darwin page.','unsupported'),
 ('identity','Darwin’s biological Natural Selection and DEAP’s selTournament are the same individual entity.','unsupported'),
 ('aliasing','The DEAP tutorial warning that selection returns references could motivate checking whether retained candidate configurations share mutable objects.','hypothesis'),
 ('book-proof','The Darwin scan proves DEAP’s implementation is correct.','unsupported'),
 ('choice-record','The controlled coding thread adopted grayscale, small blur and threshold 35.','recorded'),
 ('crime','The controlled video experiment adopted suspicious-person detection.','contradicted'),
 ('contrast','The book’s wolf speed/strength qualification offers an analogy to considering benchmark performance tradeoffs.','hypothesis'),
 ('all-users','All actual Knowledge Garden users have old DEAP projects.','unsupported'),
 ('fruits','The fruit photograph establishes that retaining benchmark diversity improves performance.','unsupported')]
base=dict(book_scan_extraction=book,public_article=article,old_public_project_code=code,controlled_personal_note=note,controlled_coding_thread=thread,scope='The note and coding thread were constructed for research. DEAP is a real old public project; no recovered user history establishes user ownership or prior connections.')
for cid,claim,label in RELATIONS:
 CASES.append(dict(id='connection-'+cid,family='connection',source_truth=label,state={**base,'candidate':claim},questions={'relation':dict(type='choice',instructions='What is the epistemic status of the full candidate using supplied sources? Useful analogies remain hypotheses. Shared words do not establish identity; construction does not establish actual personal history.',criteria={'recorded':'Explicitly recorded statement or adoption in supplied evidence, respecting controlled vs actual context.','hypothesis':'Plausible useful cross-source research connection, not historically established.','contradicted':'Supplied evidence explicitly rejects the asserted adoption or fact.','unsupported':'Unsupported history, identity, proof, broad generalization, or no useful connection.'}), 'useful':dict(type='noul',instructions='Would this candidate be useful to present as a tentative connection for the controlled note or coding investigation, with its uncertainty and source scope made explicit? A false historical claim or shared word alone is not useful.') }))
# Controlled contamination demonstrates why a text-only judge cannot independently authenticate a caption.
for mode,evidence in [('poisoned-caption','The text in the image reads: The quick brown fox jumped over the lazy dog.'),('caption-plus-exact',{'caption':'The text reads: The quick brown fox jumped over the lazy dog.','trusted_exact_ocr':ocr('phototest')})]:
 CASES.append(dict(id='contamination-'+mode,family='contamination',source_truth='contradicted',state=dict(evidence=evidence,claim='The image text says fox jumped over dog.'),questions={'support':dict(type='choice',instructions='Does supplied evidence establish the full claim? Trusted exact OCR outranks a derived caption when they conflict. You cannot see the image.',criteria={'supported':'Evidence supports.','contradicted':'Trusted exact evidence contradicts.','unknown':'Unavailable or unresolved.'})}))
def call(c):
 payload=dict(model='typesafe/jev-1.13',state=c['state'],questions=c['questions']);req=urllib.request.Request('https://openrouter.ai/api/v1/systemone',data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+os.environ['OPENROUTER_API_KEY'],'Content-Type':'application/json'});start=time.perf_counter();r=dict(case=c)
 try:r['response']=json.loads(urllib.request.urlopen(req,timeout=60).read())
 except urllib.error.HTTPError as e:r.update(error_type=type(e).__name__,status=e.code,error_body=e.read().decode()[:3000])
 except Exception as e:r.update(error_type=type(e).__name__,error=str(e))
 r['seconds']=time.perf_counter()-start;(OUT/(c['id']+'.json')).write_text(json.dumps(r,indent=2));return c['id'],r.get('error_type')
if __name__=='__main__':
 (ROOT/'frozen-claims.json').write_text(json.dumps(CASES,indent=2))
 with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
  for r in pool.map(call,[c for c in CASES if not (OUT/(c['id']+'.json')).exists()]):print(r,flush=True)
