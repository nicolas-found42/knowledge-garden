"""Controlled disk invariants, NOT an Obsidian renderer or production Markdown parser."""
from pathlib import Path
import hashlib,json,re,shutil,tempfile,time,platform,sys,difflib
ROOT=Path(__file__).resolve().parent
F=ROOT/'disposable-vault'; OUT=ROOT/'runs'; OUT.mkdir(exist_ok=True)
hashbytes=lambda b:hashlib.sha256(b).hexdigest()
def digest(p):return hashbytes(p.read_bytes())
def parse(p):
 text=p.read_text();parts=text.split('---',2)
 if len(parts)!=3 or parts[0]!='':raise ValueError('missing frontmatter')
 props={}
 for line in parts[1].strip().splitlines():
  k,v=line.split(': ',1)
  if k in props:raise ValueError('duplicate property')
  props[k]=json.loads(v)
 return props,text

def rebuild(v):
 records={};conflicts={};errors=[];assets={};links={}
 for p in sorted(v.rglob('*.md')):
  try:
   props,text=parse(p);i=props['id']; rec={'path':str(p.relative_to(v)),'props':props,'sha256':digest(p),'anchors':re.findall(r'\^([A-Za-z0-9-]+)(?:\n|$)',text)}
   if i in records: conflicts.setdefault(i,[records[i]['path']]).append(rec['path'])
   else:records[i]=rec
   links[rec['path']]=re.findall(r'\[\[([^\]|]+)',text)
   if 'asset' in props:
    a=v/props['asset'];assets[i]='missing' if not a.exists() else ('ok' if digest(a)==props['sha256'] else 'hash-mismatch')
  except (ValueError,KeyError) as e:errors.append({'path':str(p.relative_to(v)),'error':str(e)})
 for i in conflicts:records.pop(i,None)
 return {'records':records,'conflicts':conflicts,'errors':errors,'assets':assets,'links':links}

def proposal(v,page,source,new):
 pp=v/page;sp=v/source
 obj={'id':'proposal-refresh','kind':'proposal','state':'pending','target':'page-study','base_page_sha256':digest(pp),'source':'source-field','base_source_sha256':digest(sp)}
 before=pp.read_text();diff=''.join(difflib.unified_diff(before.splitlines(True),new.splitlines(True),fromfile=page,tofile=page))
 path=v/'Proposals/Refresh study.md';path.parent.mkdir(exist_ok=True)
 path.write_text('---\n'+''.join(f'{k}: {json.dumps(x)}\n' for k,x in obj.items())+'---\n\n# Review refresh\n\n```diff\n'+diff+'```\n\n## Proposed full text\n\n```markdown\n'+new+'\n```\n')
 return path,obj,new

def accept(v,p,obj,new):
 idx=rebuild(v);rec=idx['records'].get(obj['target']);source=idx['records'].get(obj['source'])
 if not rec or not source:return 'identity-conflict'
 target=v/rec['path'];sp=v/source['path']
 if digest(target)!=obj['base_page_sha256']:return 'page-conflict'
 if digest(sp)!=obj['base_source_sha256']:return 'source-conflict'
 # Sequential fixture CAS; real multiwriter safe atomic compare-and-replace is untested.
 target.write_text(new);p.write_text(p.read_text().replace('state: "pending"','state: "accepted"'));return 'accepted'
expected=json.loads((ROOT/'expected.json').read_text());results=[]
with tempfile.TemporaryDirectory(prefix='kg-durable-md-') as td:
 v=Path(td)/'vault';shutil.copytree(F,v)
 def check(name,fn):
  t=time.perf_counter()
  try:actual=bool(fn());error=None
  except Exception as e:actual=None;error=repr(e)
  results.append({'case':name,'expected':expected[name],'actual':actual,'pass':actual==expected[name],'elapsed_ms':(time.perf_counter()-t)*1000,'error':error})
 baseline=rebuild(v)
 check('baseline_durable_kinds',lambda:set(['page','source','relationship','annotation','trail','saved-search','saved-view']).issubset({x['props']['kind'] for x in baseline['records'].values()}))
 cache=v/'.cache/index.json';cache.parent.mkdir();cache.write_text(json.dumps(baseline));shutil.rmtree(cache.parent)
 check('rebuild_identical_after_cache_deleted',lambda:rebuild(v)==baseline)
 page=v/'Pages/Flower study.md';page.write_text(page.read_text()+'\nExternal editor says: compare seasons.\n')
 check('external_edit_authoritative',lambda:rebuild(v)['records']['page-study']['sha256']==digest(page) and digest(page)!=baseline['records']['page-study']['sha256'])
 mira=v/'Pages/Mira.md';moved=v/'Pages/Observer renamed.md';mira.rename(moved)
 check('move_stable_id_resolves',lambda:rebuild(v)['records']['person-mira']['path']=='Pages/Observer renamed.md')
 check('external_move_breaks_path_link',lambda:'[[Mira]]' in page.read_text() and not mira.exists())
 original_link=page.read_text();page.write_text(original_link.replace('[[Mira]]','[[Observer renamed]]'))
 check('controlled_link_repair_preserves_anchor',lambda:'[[Field observation#^observation]]' in page.read_text() and '[[Observer renamed]]' in page.read_text())
 src=v/'Sources/Field observation.md';before_src=src.read_text();src.write_text(before_src.replace('## Evidence','## Renamed evidence'))
 check('heading_rename_breaks_heading_selector',lambda:'## Evidence\n' not in src.read_text())
 src.write_text(src.read_text().replace('three red flowers','three flowers described as red'))
 check('block_anchor_survives_prose_edit',lambda:'observation' in rebuild(v)['records']['source-field']['anchors'])
 src.write_text(src.read_text().replace(' ^observation',''))
 check('removed_anchor_is_unresolved',lambda:'observation' not in rebuild(v)['records']['source-field']['anchors'])
 src.write_text(before_src)
 duplicate=v/'Pages/duplicate.md';duplicate.write_text(moved.read_text())
 check('duplicate_id_is_conflict',lambda:len(rebuild(v)['conflicts']['person-mira'])==2)
 check('duplicate_id_never_overwrites',lambda:'person-mira' not in rebuild(v)['records']);duplicate.unlink()
 check('same_bytes_distinct_sources',lambda:baseline['records']['source-field']['props']['sha256']==baseline['records']['source-second']['props']['sha256'] and len([i for i in baseline['records'] if i in ['source-field','source-second']])==2)
 check('revision_identity_is_distinct_from_blob',lambda:baseline['records']['source-field-r2']['props']['source_identity']=='source-field' and baseline['records']['source-field-r2']['props']['revision']=='r2' and baseline['records']['source-field-r2']['props']['sha256']!=baseline['records']['source-field']['props']['sha256'])
 check('historical_relationship_revision_is_not_promoted',lambda:baseline['records']['rel-observation']['props']['evidence_revision']=='r1' and baseline['records']['source-field']['props']['revision']=='r1')
 check('new_revision_distinct_hash',lambda:digest(v/'Assets/field-notes.txt')!=digest(v/'Assets/field-notes-revision.txt'))
 asset=v/'Assets/field-notes.txt';raw=asset.read_bytes();asset.unlink()
 check('missing_original_flagged',lambda:rebuild(v)['assets']['source-field']=='missing');asset.write_bytes(raw+b'corrupt')
 check('corrupt_original_flagged',lambda:rebuild(v)['assets']['source-field']=='hash-mismatch');asset.write_bytes(raw)
 old=page.read_text();new=old.replace('Three red flowers were reported','The controlled note reports three red flowers')
 p,obj,new=proposal(v,'Pages/Flower study.md','Sources/Field observation.md',new)
 check('proposal_durable_markdown',lambda:parse(p)[0]['kind']=='proposal' and '```diff' in p.read_text())
 check('proposal_does_not_mutate_page',lambda:page.read_text()==old)
 page.write_text(old+'Concurrent user line.\n')
 check('stale_page_proposal_rejected',lambda:accept(v,p,obj,new)=='page-conflict' and 'Concurrent user line.' in page.read_text());page.write_text(old)
 src.write_text(before_src+'External source edit.\n')
 check('stale_source_proposal_rejected',lambda:accept(v,p,obj,new)=='source-conflict' and page.read_text()==old);src.write_text(before_src)
 check('fresh_review_accept_preserves_user_notes',lambda:accept(v,p,obj,new)=='accepted' and 'compare seasons' in page.read_text() and 'I want to compare a later observation' in page.read_text())
 p,obj,new=proposal(v,'Pages/Flower study.md','Sources/Field observation.md',page.read_text());p.write_text(p.read_text().replace('state: "pending"','state: "rejected"'))
 check('rejected_proposal_retained',lambda:parse(p)[0]['state']=='rejected')
 bad=v/'Pages/bad.md';bad.write_text('---\nid: "bad"\nid: "different"\n---\nMalformed')
 check('malformed_properties_fail_closed',lambda:bool(rebuild(v)['errors']) and 'bad' not in rebuild(v)['records']);bad.unlink()
 moved.write_text(moved.read_text().replace('kind: "person"','kind: "person"\nexternal_custom: "keep me"'))
 check('unknown_properties_preserved',lambda:rebuild(v)['records']['person-mira']['props']['external_custom']=='keep me')
 check('all_durable_knowledge_in_md',lambda:all(x['path'].endswith('.md') for x in rebuild(v)['records'].values()))
 snapshot=rebuild(v)
 report={'timestamp_utc':__import__('datetime').datetime.now(__import__('datetime').timezone.utc).isoformat(),'environment':{'python':sys.version,'platform':platform.platform()},'expected_sha256':digest(ROOT/'expected.json'),'fixture_sha256':hashbytes(json.dumps(baseline,sort_keys=True).encode()),'results':results,'summary':{'passed':sum(r['pass'] for r in results),'total':len(results)},'final_rebuilt_index':snapshot,'scope':'Sequential isolated controlled disk test; not actual Obsidian, not general YAML parser, not atomic multiwriter CAS'}
 out=OUT/(__import__('datetime').datetime.now().strftime('%Y%m%d-%H%M%S')+'.json');out.write_text(json.dumps(report,indent=2));print(json.dumps({'result':str(out),'summary':report['summary']}))
 if not all(r['pass'] for r in results):sys.exit(1)
