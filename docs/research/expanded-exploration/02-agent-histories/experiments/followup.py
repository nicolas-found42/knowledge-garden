"""Paired real representation checks and two narrow prompt diagnostics.
Retains initial records; no fixture rewriting or hidden retries.
"""
import argparse,concurrent.futures,json,time,statistics,collections
from pathlib import Path
import run_jev as lab
IDS=['codex-9b3b7a36cc9098b6','codex-bc2b773ac01321b0','codex-70d3bee9484040f5','claude-4dc8983581a13659','claude-52ff9e3b362aa44f','claude-b5eaa298261b25bd','omp-d3292a0bbb22de81','omp-504c1fafb65fee36','omp-adfd29ff6244e9ce','hermes-2116d347594d501e','hermes-9a42078e864ae3a4','hermes-aa964ba312495394']
def main():
 p=argparse.ArgumentParser();p.add_argument('--private-dir',type=Path,required=True);args=p.parse_args();dst=lab.private_directory(args.private_dir)
 if (dst/'followup-results.json').exists():raise SystemExit('Refusing overwrite')
 prior=json.loads((dst/'jev-results.json').read_text());cases=[];retention=[]
 for r in prior:
  c=r['case']
  if c['id'] not in IDS:continue
  ms=c['state']['messages'];a=[m for m in ms if m['role']=='assistant'];conv=[m for m in ms if m['role'] in ['user','assistant']]
  for view,msgs in [('extractive_summary',a[-1:]),('selected_conversation',conv[:1]+conv[-2:] if len(conv)>3 else conv)]:
   msgs=[{**m,'text':m['text'][:4000]} for m in msgs]
   state=dict(messages=msgs,representation=view,source_representation=c['representation'])
   cases.append(dict(id=c['id']+'-'+view,family='paired_real',representation=view,state=state,questions=lab.QUESTIONS,expected=c['expected'],baseline=lab.baseline(msgs)))
   retention.append(dict(id=c['id'],view=view,source_message_count=len(ms),retained_message_ids=len({m['id'] for m in msgs}),source_chars=sum(len(m['text']) for m in ms),retained_chars=sum(len(m['text']) for m in msgs),user_messages_retained=sum(m['role']=='user' for m in msgs),tool_messages_retained=sum(m['role'] in ['tool','toolResult'] for m in msgs)))
 # Initial known failure: verification question conflated negative state with
 # failure. Diagnostic asks support of exact proposition independent of desirability.
 for r in prior:
  c=r['case']
  if c['id'] not in ['holdout-negative-full_readable','false-completion-full_readable']:continue
  q=dict(type='choice',instructions='Does actual supplied tool/measurement evidence SUPPORT the EXACT proposition in target_verification? A negative proposition such as scheduler disabled=true may be verified even though it describes a disabled capability. Ignore desirability and whether this was the users preferred result. Assistant claims alone are insufficient.',criteria={'verified':'Actual evidence supports the exact target_verification proposition.','failed':'Actual evidence contradicts the exact proposition.','unknown':'No actual decisive evidence about this proposition.'})
  cases.append({**c,'id':c['id']+'-diagnostic','family':'verification_diagnostic','questions':{'verification':q},'expected':{'verification':c['expected']['verification']}})
 (dst/'followup-cases.json').write_text(json.dumps(cases,indent=2));start=time.perf_counter()
 with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:results=list(pool.map(lab.call,cases))
 (dst/'followup-results.json').write_text(json.dumps(results,indent=2));(dst/'retention.json').write_text(json.dumps(retention,indent=2))
 ok=[r for r in results if 'response'in r];usage=[r['response']['usage'] for r in ok];metrics={}
 for family,view in sorted({(r['case']['family'],r['case']['representation']) for r in ok}):
  rs=[r for r in ok if (r['case']['family'],r['case']['representation'])==(family,view)]
  checks=[(e,r['response']['answers'][k]['choice']) for r in rs for k,e in r['case']['expected'].items()]
  metrics[family+'/'+view]=dict(calls=len(rs),source_label_agreement=sum(e==a for e,a in checks),checks=len(checks),errors=[dict(id=r['case']['id'],expected=e,actual=r['response']['answers'][k]['choice']) for r in rs for k,e in r['case']['expected'].items() if e!=r['response']['answers'][k]['choice']])
 summary=dict(calls=len(results),successful=len(ok),failures=len(results)-len(ok),input_tokens=sum(u['input_tokens'] for u in usage),output_tokens=sum(u['output_tokens'] for u in usage),cost_usd=sum(u.get('cost',0) for u in usage),model_ids=sorted({r['response']['model'] for r in ok}),median_seconds=statistics.median(r['seconds'] for r in ok),p95_seconds=sorted(r['seconds'] for r in ok)[int(.95*(len(ok)-1))],wall_seconds=time.perf_counter()-start,metrics=metrics,retention=retention)
 (dst/'followup-summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
