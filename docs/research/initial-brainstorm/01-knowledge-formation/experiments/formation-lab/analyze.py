"""Offline composition and metric audit. No model calls."""
import collections,itertools,json
from pathlib import Path
from datetime import date
ROOT=Path(__file__).parent

def complete_link(nodes,probabilities,threshold=.9):
 """Merge clusters only when EVERY cross-pair is explicit high-confidence identity."""
 groups=[{n} for n in sorted(nodes)]
 changed=True
 while changed:
  changed=False
  for i,j in itertools.combinations(range(len(groups)),2):
   if all(probabilities.get(tuple(sorted((a,b))),0)>=threshold for a in groups[i] for b in groups[j]):
    groups[i]|=groups[j];groups.pop(j);changed=True;break
 return [sorted(g) for g in groups]

def policy(r,threshold=.9):
 c=r['case'];a=r['response']['answers'];fam=c['family']
 if fam=='coverage':return a['coverage']['choice']=='supports' and a['complete']['noul']>=threshold
 if fam=='entity':return a['identity']['choice']=='same' and a['same']['noul']>=threshold
 if fam=='lifecycle':
  newer=date.fromisoformat(c['state']['new_date'])>date.fromisoformat(c['state']['old_date'])
  return newer and a['relation']['choice'] in ['replace','retract'] and a['explicit']['noul']>=threshold
 return None

def selfcheck():
 # A--B and B--C semantic similarities must never silently assert A--C identity.
 p={('A','B'):.99,('B','C'):.99,('A','C'):.1}
 g=complete_link(['A','B','C'],p)
 assert len(g)==2 and not any({'A','C'}<=set(x) for x in g)
 assert complete_link(['C','B','A'],p)==g
 assert complete_link(['A','B'],{})==[['A'],['B']]
 assert date.fromisoformat('2026-10-01')>date.fromisoformat('2026-09-30')
 assert not date.fromisoformat('2026-09-03')>date.fromisoformat('2026-09-03')
 for line in ['Keep original scans.','Do not use Redis.']:
  source='Before\n'+line+'\nAfter';start=source.index(line);assert source[start:start+len(line)]==line
 return dict(checks=7,bridge_probabilities={str(k):v for k,v in p.items()},bridge_groups=g)

def main():
 data=json.loads((ROOT/'results.json').read_text());ok=[r for r in data['results'] if 'response'in r]
 agg=collections.defaultdict(lambda:dict(requests=0,correct=0,checks=0,exact_case=0))
 failures=[];pol=[]
 for r in ok:
  c=r['case'];key=c['family']+'/'+c['split']+'/'+c['variant'];x=agg[key];x['requests']+=1;x['checks']+=len(r['checks']);x['correct']+=sum(v['correct'] for v in r['checks'].values());x['exact_case']+=all(v['correct'] for v in r['checks'].values())
  if not all(v['correct'] for v in r['checks'].values()): failures.append(dict(family=c['family'],name=c['name'],variant=c['variant'],checks=r['checks'],answers=r['response']['answers']))
 for fam in ['coverage','entity','lifecycle']:
  for split in ['development','holdout']:
   rows=[r for r in ok if r['case']['family']==fam and r['case']['split']==split]
   for t in [.5,.8,.9,.99]:
    preds=[policy(r,t) for r in rows];truth=[r['case']['policy'] for r in rows]
    pol.append(dict(family=fam,split=split,threshold=t,n=len(rows),actions=sum(preds),false_actions=sum(p and not y for p,y in zip(preds,truth)),missed_actions=sum(y and not p for p,y in zip(preds,truth))))
 # Separate plain keyword baseline, independent of model output.
 baseline=[]
 for fam in ['coverage','entity','ontology']:
  rows=[r for r in ok if r['case']['family']==fam and r['case']['variant']=='clean'];correct=0
  for r in rows:
   s=r['case']['state'];expected=r['case']['expected']
   if fam=='coverage':pred='supports' if s['claim'] in ' '.join(s['sources'].values()) else 'unsupported';target=expected['coverage']
   elif fam=='entity':pred='same' if s['left']==s['right'] else 'different';target=expected['identity']
   else:
    text=s['text'].lower();pred=next((k for k,words in [('storage',['markdown','files','storage']),('retrieval',['search','retriev']),('identity',['alias','entit']),('governance',['review','evidence'])] if any(w in text for w in words)),'none');target=expected['topic']
   correct+=pred==target
  baseline.append(dict(family=fam,n=len(rows),correct=correct))
 # Same source, unchanged semantic questions, reusable score produces four offline views.
 views={}
 for weights in [(1,0),(0,1),(.7,.3),(.3,.7)]:
  rows=[r for r in ok if r['case']['family']=='coverage' and r['case']['variant']=='clean']
  ranked=sorted(rows,key=lambda r:weights[0]*r['response']['answers']['quality']['score']/2+weights[1]*r['response']['answers']['complete']['noul'],reverse=True)
  views[str(weights)]=[r['case']['name'] for r in ranked[:5]]
 result=dict(summary=data['summary'],by_family_split_variant=dict(agg),policy=pol,failures=failures,lexical_baselines=baseline,offline_views_no_new_inference=views,invariants=selfcheck())
 (ROOT/'analysis.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({k:v for k,v in result.items() if k not in ['failures']},indent=2));print('FAILURES',len(failures));print([(x['family'],x['name'],x['variant'],{k:(v['expected'],v['actual']) for k,v in x['checks'].items() if not v['correct']}) for x in failures])
if __name__=='__main__':main()
