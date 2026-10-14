"""Explicit facet stopping after premature broad graph completion; development extension."""
import json
from run import ROOT,execute
from followup import NODES
from typesafe_sdk import Noul,Choice
out=ROOT/'results-facets.json'
if out.exists():raise SystemExit('No overwrites.')
path=['garden'];records=[];reason='budget'
for hop in range(5):
 options={k:{'label':v,'preview':NODES[k]['text']}for k,v in NODES[path[-1]]['links'].items()if k not in path}
 state={'goal':'Find BOTH why original transcripts are preserved AND the concrete method connecting quotations to their stored source body (message IDs, offsets, or equivalent traceable references).','evidence_seen':{k:NODES[k]['text']for k in path},'observed_outgoing':options}
 questions={'preservation_reason':Noul(instructions='Does `evidence_seen` explicitly explain the failure or reason motivating preservation of original transcript revisions? A statement to preserve them without why is insufficient.'),'quote_traceability':Noul(instructions='Does `evidence_seen` explicitly describe a concrete method connecting quoted text to its saved original source, such as saved content hashes PLUS source message IDs or offsets? Generic preservation alone is insufficient.'),'next':Choice(instructions='Choose an observed outgoing link likely to supply a missing requirement from `goal`. Prioritize concrete traceability mechanics once the preservation reason is known. Select stop if no useful link remains.',criteria=options|{'stop':'No observed useful link.'})}
 expected={'preservation_reason':'revisions'in path,'quote_traceability':'mechanics'in path}
 r=execute(dict(family='graph_facets',name='_'.join(path),state=state,questions=questions,expected=expected));records.append(r)
 if 'response'not in r:reason='service_error';break
 a=r['response']['answers']
 if all(a[k]['noul']>=.7 for k in ['preservation_reason','quote_traceability']):reason='all_facets_pass';break
 nxt=a['next']['choice']
 if nxt=='stop':reason='no_useful_link';break
 if nxt not in options:raise AssertionError('Unobserved edge')
 path.append(nxt)
out.write_text(json.dumps({'development_status':'Added after inspecting premature broad-completion result; explicit facets alter the goal contract and prompts. No holdout.','path':path,'reason':reason,'gold_evidence_coverage':{'preservation_reason':'revisions'in path,'quote_traceability':'mechanics'in path},'results':records},indent=2)+'\n')
print(json.dumps({'calls':len(records),'path':path,'reason':reason}))
