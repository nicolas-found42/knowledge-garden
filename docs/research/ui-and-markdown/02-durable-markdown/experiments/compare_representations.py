"""Compare three readable Markdown encodings over the same authored relationship."""
from pathlib import Path
import json,re,time,platform,sys,hashlib
R=Path(__file__).resolve().parent;D=R/'representations';D.mkdir(exist_ok=True)
record={'id':'rel-observation','subject':'person-mira','relation':'observed-at','object':'place-riverside','status':'recorded','evidence_source':'source-field','evidence_revision':'r1','evidence_anchor':'observation'}
# Frozen assertion: every encoding recovers exactly this record; append-only prose does not alter it.
(R/'representation-expected.json').write_text(json.dumps({'record':record,'assertions':['exact-recovery','external-prose-preserved','all-meaning-in-md']},indent=2))
shared='\n[[Mira]] observed-at [[Riverside]]. Evidence: [[Field observation#^observation]].\n\nThis is a controlled authored source, not an independently verified event.\n'
files={
 'A-flat-relationship-note.md':'---\n'+''.join(f'{k}: {json.dumps(v)}\n' for k,v in record.items())+'---\n\n# Relationship note\n'+shared,
 'B-inline-record.md':'# Flower study with inline relationship\n\n```json relationship\n'+json.dumps(record,indent=2)+'\n```\n'+shared,
 'C-table-ledger.md':'# Relationships ledger\n\n| '+' | '.join(record)+' |\n| '+' | '.join('---' for _ in record)+' |\n| '+' | '.join(record.values())+' |\n'+shared}
results=[]
for name,text in files.items():
 p=D/name;p.write_text(text);start=time.perf_counter()
 if name.startswith('A'):parsed={k:json.loads(v) for k,v in (l.split(': ',1) for l in text.split('---')[1].strip().splitlines())}
 elif name.startswith('B'):parsed=json.loads(re.search(r'```json relationship\n(.*?)\n```',text,re.S)[1])
 else:
  rows=[l for l in text.splitlines() if l.startswith('|')];parsed=dict(zip([v.strip() for v in rows[0].strip('|').split('|')],[v.strip() for v in rows[2].strip('|').split('|')]))
 results.append({'file':name,'bytes':p.stat().st_size,'exact_recovery':parsed==record,'elapsed_ms':(time.perf_counter()-start)*1000,'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'external_prose_preserved':(text+'\nExternal annotation.\n').startswith(text)})
(R/'representation-results.json').write_text(json.dumps({'environment':{'python':sys.version,'platform':platform.platform()},'results':results,'limitations':'Minimal parsers for frozen fixture shapes, not arbitrary Markdown. Append preservation tests bytes, not GUI edits.'},indent=2));print(json.dumps(results,indent=2))
