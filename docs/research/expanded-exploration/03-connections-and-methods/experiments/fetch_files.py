import json,pathlib,urllib.request,hashlib
R=pathlib.Path(__file__).parent/'sources';trees=json.loads((R/'trees.json').read_text());selected={'devagrawal09/stanley-code':['src/workflows/triage-failures.ts','src/workflows/check-task.ts','src/core/workflow.ts','docs/workflow-design-decisions.md'],'shimo4228/jev-research-pipeline':['src/jev_research_pipeline/jev/question_movement.py','src/jev_research_pipeline/jev/source_support.py','src/jev_research_pipeline/model/nodes.py','src/jev_research_pipeline/store/stage_cache.py'],'kitze/skillbox':['src/server/mcp.ts','src/server/skill-resources.ts','src/skill-manifest.ts','src/skill-import-safety.ts'],'doeixd/discern':['src/procedure.ts','test/procedure.test.mjs','README.md']};out=[]
for t in trees:
 row={'repo':t['repo'],'sha':t['sha'],'files':[]}
 for p in selected[t['repo']]:
  if p not in t['paths']:
   print('missing',t['repo'],p);continue
  u='https://raw.githubusercontent.com/'+t['repo']+'/'+t['sha']+'/'+p;raw=urllib.request.urlopen(u,timeout=40).read();dest=R/(t['repo'].replace('/','__'))/p;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(raw);row['files'].append({'path':p,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'url':'https://github.com/'+t['repo']+'/blob/'+t['sha']+'/'+p}); print(t['repo'],p,len(raw))
 out.append(row)
(R/'manifest.json').write_text(json.dumps(out,indent=2))
