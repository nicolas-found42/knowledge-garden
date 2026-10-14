"""Post-result diagnostics; NOT unedited holdout evidence."""
import concurrent.futures,json,time
from pathlib import Path
import run
from typesafe_sdk import Noul
ROOT=Path(__file__).parent
rows=[]
for c in run.CASES:
 if c['family']!='entity':continue
 c={**c,'split':'post_result_diagnostic','variant':'explicit_references','questions':{
 'original':Noul(instructions='Does evidence establish individual identity, beyond a shared ambiguous name?'),
 'named':Noul(instructions='Do `left` and `right` denote the same individual entity? Require an explicit alias, rename or shared canonical identifier. Shared names alone do not establish identity. Components and independent forks are different.'),
 'inline':Noul(instructions=f'Do these TWO supplied descriptions identify the same individual entity: LEFT {c["state"]["left"]!r}; RIGHT {c["state"]["right"]!r}? Explicit alias, rename or shared canonical identifier counts. Ambiguous names alone do not.'),
 },'expected':{},'policy':c['policy']}
 rows.append(c)
start=time.perf_counter()
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool: results=list(pool.map(run.run_case,rows))
summary={key:{'correct_at_0.5':sum((r['response']['answers'][key]['noul']>=.5)==r['case']['policy'] for r in results),'n':len(results),'false_positive':sum(r['response']['answers'][key]['noul']>=.5 and not r['case']['policy'] for r in results),'false_negative':sum(r['response']['answers'][key]['noul']<.5 and r['case']['policy'] for r in results)} for key in ['original','named','inline']}
us=[r['response']['usage'] for r in results]
summary['usage']=dict(requests=len(results),input_tokens=sum(u['input_tokens']for u in us),output_tokens=sum(u['output_tokens']for u in us),cost_usd=sum(u['cost']for u in us),wall_seconds=time.perf_counter()-start)
p=ROOT/'diagnostic-results.json'
if p.exists():raise SystemExit('Refuse overwrite')
p.write_text(json.dumps(dict(summary=summary,results=results),indent=2)+'\n');print(json.dumps(summary,indent=2))
