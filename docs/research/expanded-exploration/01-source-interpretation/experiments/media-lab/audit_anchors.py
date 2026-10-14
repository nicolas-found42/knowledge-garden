"""Structural anchors are necessary, never sufficient proof of claim support."""
import re,json,collections
from pathlib import Path
ROOT=Path(__file__).parent;rows=[]
for f in sorted((ROOT/'perception-records').glob('*.json')):
 r=json.loads(f.read_text());s=r['response']['choices'][0]['message']['content'];s=re.sub(r'^```(?:json)?\s*|\s*```$','',s.strip())
 try:d=json.loads(s)
 except Exception:rows.append(dict(id=r['task']['id'],parse=False));continue
 obs=d.get('observations',[]);valid=0;anchors=[]
 for o in obs:
  a=o.get('anchor');status='missing'
  if isinstance(a,list) and len(a)==4 and all(isinstance(v,(int,float)) for v in a):
   x,y,w,h=a;status='normalized_in_bounds' if min(a)>=0 and max(a)<=1 and x+w<=1.0001 and y+h<=1.0001 else 'invalid_normalized_box'
  elif isinstance(a,str):status='source_label_without_region'
  elif isinstance(a,dict):status='uncontracted_object_box'
  elif isinstance(o.get('time_seconds'),(int,float)):status='numeric_time_in_range' if 0<=o['time_seconds']<=10 else 'invalid_time'
  anchors.append(dict(claim=o.get('claim'),status=status,anchor=a,time_seconds=o.get('time_seconds')))
 rows.append(dict(id=r['task']['id'],parse=True,observations=len(obs),anchor_checks=anchors))
(ROOT/'anchor-audit.json').write_text(json.dumps(rows,indent=2));print(json.dumps([dict(id=r['id'],parse=r['parse'],anchor_types=dict(collections.Counter(a['status'] for a in r.get('anchor_checks',[])))) for r in rows],indent=2))
