"""Different supporting model/repair outcomes; paired labels from original frozen corpus."""
import json,concurrent.futures
from run_judgments import CASES,call,text_record
from pathlib import Path
ROOT=Path(__file__).parent
cases=[]
for c in CASES:
 if c['family']!='claim' or c['representation']!='observation':continue
 source=c['source']
 if source not in ['phototest','book-pdf-page-92','video-contact-sheet','controlled-demo-screenshot']:continue
 cases.append({**c,'id':c['id']+'-gpt4omini','representation':'gpt4omini-observation','state':{**c['state'],'evidence':text_record(source+'-observation-gpt4omini')}})
for c in CASES:
 if c['family']=='claim' and c['representation']=='observation' and c['source']=='controlled-demo-screenshot':
  cases.append({**c,'id':c['id']+'-bounded-repair','representation':'bounded-repair','state':{**c['state'],'evidence':text_record(c['source']+'-observation-bounded-repair')}})
(ROOT/'diagnostic-judgment-labels.json').write_text(json.dumps(cases,indent=2))
if __name__=='__main__':
 with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
  for out in pool.map(call,cases):print(out,flush=True)
