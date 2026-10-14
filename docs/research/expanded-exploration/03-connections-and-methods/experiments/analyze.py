import json,pathlib,statistics,re
R=pathlib.Path(__file__).parent;cs=json.loads((R/'cases.json').read_text());results=json.loads((R/'results.json').read_text())['results'];by={c['id']:c for c in cs};checks=[]
for r in results:
 if 'response'not in r:continue
 if r['id']in by:
  c=by[r['id']];a=r['response']['answers'];checks.append({'id':c['id'],'engine':'jev','split':c['split'],'family':c['family'],'expected':c['expected'],'actual':a['relation']['choice'],'correct':a['relation']['choice']==c['expected'],'expected_useful':c['expected_useful'],'useful_probability':a['useful']['noul'],'useful_at_0_5':a['useful']['noul']>=.5})
 elif r['id'].startswith('baseline-'):
  try:
   rows=json.loads(r['response']['choices'][0]['message']['content'])['results']
   for a in rows:
    c=by[a['id']];checks.append({'id':c['id'],'engine':'generative','split':c['split'],'family':c['family'],'expected':c['expected'],'actual':a['relation'],'correct':a['relation']==c['expected'],'expected_useful':c['expected_useful'],'useful':a['useful'],'quote_exact':a['evidence_quote']in c['state']['left']or a['evidence_quote']in c['state']['right']})
  except Exception as e:checks.append({'id':r['id'],'parse_error':str(e)})
# Transparent lexical shortlist, not a strong semantic model. All unique pair cases broad evaluated above.
# Honest deterministic conservative baseline: exact canonical repo fields for identity; otherwise abstain.
def det(c):
 if c['family']=='identity':
  refs=lambda s:set(re.findall(r'\b[a-z]+/[a-z]+\b',s));l=refs(c['state']['left']);r=refs(c['state']['right'])
  if l and r and l.isdisjoint(r):return 'different'
  if l and l==r and 'module'not in c['state']['left']and 'fork'not in c['state']['left']:return 'same'
  return 'uncertain'
 return {'decision':'unestablished','evidence':'silent','method':'insufficient','analogy':'low_value'}[c['family']]
for c in cs:checks.append({'id':c['id'],'engine':'deterministic_conservative','split':c['split'],'family':c['family'],'expected':c['expected'],'actual':det(c),'correct':det(c)==c['expected']})
ledger={}
for engine in ['jev','generative']:
 rs=[r for r in results if r['engine']==engine and 'response'in r];us=[r['response'].get('usage',{})for r in rs];ledger[engine]={'requests':len(rs),'models':sorted({r['response']['model']for r in rs}),'input_tokens':sum(u.get('input_tokens',u.get('prompt_tokens',0))for u in us),'output_tokens':sum(u.get('output_tokens',u.get('completion_tokens',0))for u in us),'reported_cost_usd':sum(u.get('cost',0)for u in us),'all_cost_available':all('cost'in u for u in us),'median_seconds':statistics.median(r['seconds']for r in rs),'total_request_seconds':sum(r['seconds']for r in rs)}
summary={'ledger':ledger,'failures':[r for r in results if 'response'not in r],'comparison':{},'checks':checks}
for engine in ['jev','generative','deterministic_conservative']:
 summary['comparison'][engine]={}
 for split in ['development','holdout']:
  rows=[c for c in checks if c.get('engine')==engine and c.get('split')==split];summary['comparison'][engine][split]={'correct':sum(c['correct']for c in rows),'total':len(rows)}
(R/'analysis.json').write_text(json.dumps(summary,indent=2));print(json.dumps({k:v for k,v in summary.items()if k!='checks'},indent=2));print('errors',json.dumps([c for c in checks if c.get('correct')==False],indent=2))
