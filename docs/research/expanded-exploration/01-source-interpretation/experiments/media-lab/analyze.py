"""Offline observed cost/representation/anchor accounting, no inference."""
import json,re,collections
from pathlib import Path
ROOT=Path(__file__).parent
summary={};by=collections.defaultdict(lambda:dict(total=0,source_truth_agreement=0,misses=[],unsupported_promotions=[]));ledger={}
for kind,folder in [('supporting_model','perception-records'),('jev','judgment-records')]:
 rows=[json.loads(f.read_text()) for f in (ROOT/folder).glob('*.json')];successful=[r for r in rows if 'response' in r];us=[r['response'].get('usage',{}) for r in successful]
 ledger[kind]=dict(calls=len(rows),successful=len(successful),failures=len(rows)-len(successful),models=sorted({r['response'].get('model','unknown') for r in successful}),router_reported_cost=sum(x.get('cost',0) or 0 for x in us),upstream_reported_inference_cost=sum(x.get('cost_details',{}).get('upstream_inference_cost',0) or 0 for x in us),byok_calls=sum(bool(x.get('is_byok')) for x in us),input_tokens=sum(x.get('prompt_tokens',x.get('input_tokens',0)) for x in us),output_tokens=sum(x.get('completion_tokens',x.get('output_tokens',0)) for x in us))
 if kind=='jev':
  for r in successful:
   c=r['case'];key=c.get('representation',c['family']);v=by[key];v['total']+=1;answer=r['response']['answers'].get('support',r['response']['answers'].get('relation',{})).get('choice');v['source_truth_agreement']+=answer==c['source_truth']
   if answer!=c['source_truth']:v['misses'].append(dict(id=c['id'],source_truth=c['source_truth'],actual=answer))
   if c['source_truth'] in ['unknown','unsupported','contradicted'] and answer in ['supported','recorded']:v['unsupported_promotions'].append(c['id'])
  summary['connection_answers']=[dict(id=r['case']['id'],expected=r['case']['source_truth'],answers=r['response']['answers']) for r in successful if r['case']['family']=='connection']
 else:
  parses=[]
  for r in successful:
   choice=r['response'].get('choices',[{}])[0];s=choice.get('message',{}).get('content','');s=re.sub(r'^```(?:json)?\s*|\s*```$','',s.strip())
   try:d=json.loads(s);status='valid_json'
   except Exception:status='invalid_json'
   parses.append(dict(id=r['task']['id'],finish_reason=choice.get('finish_reason'),parse_status=status))
  summary['generation_output_integrity']=parses
summary.update(ledger=ledger,representations=dict(by),metric_note='Source-truth agreement compares original manually inspected public media to Jev over converted text. Misses can be conversion losses or correct abstention on unavailable evidence; this is not a pure Jev accuracy metric.')
(ROOT/'summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
