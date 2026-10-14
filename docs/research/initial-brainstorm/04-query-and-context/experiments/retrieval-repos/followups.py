"""Preserve original errors; explicit service retry, branch-specific binding, actual source acquisition."""
import json,concurrent.futures,time,argparse
from pathlib import Path
import run as lab
from typesafe_sdk import Choice
ROOT=Path(__file__).parent
parser=argparse.ArgumentParser();parser.add_argument('--initial',type=Path,default=ROOT/'results-initial');parser.add_argument('--out',type=Path,default=ROOT/'results-followup');args=parser.parse_args()
initial=list(map(json.loads,(args.initial/'raw.jsonl').read_text().splitlines()))
out=args.out;out.mkdir(exist_ok=False)
cases=[]
for r in initial:
 if 'error_type' in r:
  c=next(c for c in lab.CASES if (c['family'],c['name'],c['variant'])==(r['family'],r['name'],r['variant']));cases.append(c|{'variant':c['variant']+'_service_retry'})
for c in lab.CASES:
 if c['family']=='typed_query_plan' and c['variant']=='typed':
  sources=c['state']['sources'];single={x:'This source ID.' for x in sources}|{'none':'No available valid source.'};pairs={'+'.join(sources):'Both revisions in order.'} if len(sources)==2 else {};pairs['none']='No pair available.'
  q={'operator':c['questions']['operator'],'lookup_argument':Choice(instructions='If lookup were selected, which named source should be read? none if request is not a valid lookup.',criteria=single),'follow_argument':Choice(instructions='If follow were selected, which source is the START NODE from which outgoing links are followed? Choose start rather than destination. none if request is not a valid graph-follow operation.',criteria=single),'compare_argument':Choice(instructions='If compare were selected, which two source revision IDs must be compared? none if request is not compare or pair unavailable.',criteria=pairs)}
  # Code binds select=all and abstain=none; unused speculative branch outputs have no effect.
  expected={'operator':c['expected']['operator']};op=c['expected']['operator']
  if op in ['lookup','follow','compare']:expected[op+'_argument']=c['expected']['argument']
  cases.append(c|{'variant':'branch_specific','questions':q,'expected':expected})
# Retry and speculative branches are independent; source acquisition below is sequentially dependent.
results=[]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
 for r in pool.map(lab.run,cases):results.append(r)
# Finish every synopsis result, including the service-failed one recovered above.
synopsis=[r for r in initial if r['family']=='raw_source_acquisition' and r['variant']=='synopsis' and 'response' in r]
synopsis += [r for r in results if r['family']=='raw_source_acquisition' and r['variant']=='synopsis_service_retry']
for previous in synopsis:
 action=previous['response']['answers']['action']['choice']
 if action=='fetch':
  c=next(c for c in lab.CASES if c['family']=='raw_source_acquisition' and c['name']==previous['name'] and c['variant']=='raw_source')
  r=lab.run(c|{'variant':'adaptive_fetched'});r['prior_action']=action;results.append(r)
 else:
  results.append({'family':'adaptive_policy','name':previous['name'],'variant':'no_fetch','prior_action':action,'prior_answer':previous['response']['answers']['answer']['choice']})
for r in results:
 if r['family']=='typed_query_plan' and r['variant']=='branch_specific':
  op=r['response']['answers']['operator']['choice'];arg= 'all' if op=='select' else 'none' if op=='abstain' else r['response']['answers'][op+'_argument']['choice']
  valid=(op=='select' and arg=='all') or (op=='abstain' and arg=='none') or (op=='lookup' and arg in r['state']['sources']) or (op=='follow' and arg in r['state']['sources'] and 'links to' in r['state']['sources'][arg]) or (op=='compare' and len(arg.split('+'))==2 and all(x in r['state']['sources'] for x in arg.split('+')))
  r['executed_plan']={'operator':op,'argument':arg,'valid':valid,'result':r['state']['sources'] if op=='select' else None if op=='abstain' else {x:r['state']['sources'][x] for x in arg.split('+')} if valid else 'rejected'}
(out/'raw.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in results))
summary={'requests':sum('response' in r for r in results),'reported_cost_usd':sum(r['response']['usage'].get('cost',0) for r in results if 'response' in r),'groups':{}}
for r in results:
 if 'response' not in r:continue
 g=summary['groups'].setdefault(r['family']+'/'+r['variant'],{'requests':0,'checks':0,'correct':0});g['requests']+=1;g['checks']+=len(r['checks']);g['correct']+=sum(c['correct'] for c in r['checks'].values())
(out/'summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
