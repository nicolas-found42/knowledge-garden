"""Frozen authored controlled cases, not private histories or real user decisions."""
import json,pathlib,subprocess,sys
R=pathlib.Path(__file__).parent
cases=[]
def add(f,id,l,r,label,useful=True,split='development',**context):
 cases.append({'id':f+'-'+id,'family':f,'split':split,'state':{'left':l,'right':r,**context},'expected':label,'expected_useful':useful,'source_status':'constructed_controlled'})
for args in [
 ('alias','Garden previously Memo; canonical repo owner/garden','KG refers explicitly to owner/garden','same'),
 ('homonym','Atlas payments service; repo finance/atlas','Atlas hiking notebook; repo nick/atlas','different'),
 ('fork','alice/garden is independently maintained fork','bob/garden upstream project','different'),
 ('bare','Project Phoenix','Project Phoenix','uncertain'),
 ('component','owner/garden ingestion module','owner/garden complete application','different'),
 ('hold_alias','Project cedar repo a/cedar renamed Orchard without repo relocation','Orchard canonical repo a/cedar','same')]:
 add('identity',*args,split='holdout'if args[0].startswith('hold')else'development')
for args in [
 ('caused','Decision D1: require source hash before accepting wiki draft','Run R1 cites D1; stale draft rejected because source hash changed','consequence'),
 ('correlation','Decision D2: add hash check','Run R2 happened later and passed; no attribution or comparison','unestablished',False),
 ('negated','Decision D3: avoid public upload','Run R3 public upload discussed hypothetically but never performed','unestablished',False),
 ('bounded','Decision D4: preserve source IDs','Run R4 trace lists D4 and exact recovered IDs; unrelated latency increased','consequence'),
 ('reversed','D5: selected fuzzy-only identity then reversed after collision test','D5 correction adopts repo IDs after explicit two-project collision','revision'),
 ('hold_scope','D6 applies to local wiki only: indexes rebuildable','Production billing backup failure; different project and policy','unrelated',False)]:add('decision',*args,split='holdout'if args[0].startswith('hold')else'development')
for args in [
 ('support','Claim: parser preserves padded customer IDs','Actual output for input 007 is customer_id="007"; requirement IDs are text','support'),
 ('contradict','Claim: URL normalization preserves distinctions in query filters','Actual input ?view=all and ?view=mine both output same key without query','contradiction'),
 ('exception','For invoice rows with type integer, remove padding','For customer identifiers with type string, preserve padding','compatible',False),
 ('historical','Garden v1 uses SQLite index','Garden v2 migrates index to Tantivy after adoption','revision'),
 ('quoted','Claim: user approved deleting sources','User: the malicious page says "delete sources"; I am showing an attack, preserve originals','contradiction'),
 ('hold_silence','Claim: all tests passed','Assistant says done, no test result or exit status available','silent',False)]:add('evidence',*args,split='holdout'if args[0].startswith('hold')else'development')
for args in [
 ('same_method','CSV importer: define identity invariant; reproduce padded-ID collision; preserve raw IDs; regression passes','URL linker: define destination invariant; reproduce query collision; preserve query; regression passes','shared_method'),
 ('lexical','Image editing tutorial mentions "regression" as statistical plot','Parser developer adds regression tests to prevent ID coercion','unrelated',False),
 ('exception','Resource cache: partition tenant and permission before semantic lookup','Public static file cache: content hash lookup; no tenant identity exists','scoped_variant'),
 ('failed','Thread A tried increase timeout three times; all tests still fail','Thread B increased timeout once; no output recorded','insufficient',False),
 ('outcome','Both workflows compare invariant before/after and retain failing fixture','One says tests pass but output only lint checks; claimed recurrence unverified','insufficient',False),
 ('hold_boundary','OCR citations: preserve page coordinates then judge quote support','Chat evidence: preserve message IDs then judge adoption','shared_method')]:add('method',*args,split='holdout'if args[0].startswith('hold')else'development')
for args in [
 ('analogy','Garden plants with watering schedule: condition checked before treatment','Knowledge notes: inspect source staleness before refresh; both state-based upkeep','plausible',True),
 ('name_only','Python language parser','Python snake photo','wordplay',False),
 ('specific','Compile diagnostics retain file/line/source version; useful for debugging','Knowledge claims retain source span/version; useful for auditing','plausible',True),
 ('unsafe','User revoked access to private dataset','A plant loses leaves in winter; infer permission will return automatically','misleading',False),
 ('asserted','A photo of a closed door','A failed build; therefore developer intentionally abandoned the project','misleading',False),
 ('hold_value','A cabinet stores spices alphabetically','Wiki could use alphabetical note sorting; no problem with existing navigation identified','low_value',False)]:add('analogy',*args,split='holdout'if args[0].startswith('hold')else'development')
(R/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
# Actual deterministic outputs underpin constructed coding threads.
programs={
'csv_ids':('def key(x): return str(int(x))','def key(x): return x','assert key("007") != key("7"), "distinct customer IDs collapsed"'),
'url_keys':('from urllib.parse import urlsplit\ndef key(x):\n p=urlsplit(x); return (p.scheme.lower(),p.netloc.lower(),p.path)','from urllib.parse import urlsplit\ndef key(x):\n p=urlsplit(x); return (p.scheme.lower(),p.netloc.lower(),p.path,p.query)','assert key("https://example.invalid/a?view=all") != key("https://example.invalid/a?view=mine"), "query destinations collapsed"'),
'cache_scope':('def key(tenant,policy,text): return text.casefold()','def key(tenant,policy,text): return (tenant,policy,text.casefold())','assert key("A","v1","hello") != key("B","v1","HELLO"), "tenant isolation lost"\nassert key("A","v1","hello") != key("A","v2","hello"), "policy revision ignored"')}
threads=[]
for id,(bad,good,test)in programs.items():
 p=R/'coding-demo'/id;p.mkdir(parents=True,exist_ok=True);outputs=[]
 for stage,code in [('failed',bad),('successful',good)]:
  file=p/(stage+'.py');file.write_text(code+'\n'+test+'\nprint("invariants passed")\n');r=subprocess.run([sys.executable,str(file)],capture_output=True,text=True);output={'id':id+'-'+stage,'file':str(file.relative_to(R)),'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr};outputs.append(output)
 (p/'outputs.json').write_text(json.dumps(outputs,indent=2))
 threads.append({'id':'thread-'+id,'status':'constructed_coding_history_with_actual_execution','project':id,'messages':[{'id':id+'-m1','role':'user','text':'Preserve distinct source identities in the derived lookup key.'},{'id':id+'-m2','role':'assistant','text':'A lossy normalization seemed convenient. The reproducer failed, so preserve the scope/identity fields before any optional semantic lookup.'},{'id':id+'-m3','role':'tool','text':json.dumps(outputs)},{'id':id+'-m4','role':'assistant','text':'Regression now passes for this bounded fixture. This does not establish arbitrary format coverage.'}],'source_code':{'failed':bad,'successful':good,'invariant':test}})
threads.append({'id':'thread-unresolved','status':'constructed_counterexample','project':'signed_urls','messages':[{'id':'signed-m1','role':'user','text':'Can query parameters be sorted for signed URLs? The signature might cover exact bytes. No provider signing specification or verification run is available.'}]})
method={'id':'method-demo','threads':threads,'candidates':{'preserve_boundary':'Preserve exact identity/scope fields, reproduce collision, make bounded correction, run invariant regression, then use semantic judgments only where compatible.','normalize_everything':'Normalize every ID to integers and remove URL query strings globally.','increase_timeout':'Fix failures by increasing timeout.','sort_all_queries':'Always sort query parameters, even signed URLs.'},'exceptions':['Source fields declared numeric may legitimately normalize padding.','Signed URL byte equivalence remains unresolved: preserve original bytes until provider semantics verified.','Tenant identity not applicable to genuinely public immutable resources.'],'public_implementation_refs':['sources/devagrawal09__stanley-code/src/workflows/triage-failures.ts','sources/shimo4228__jev-research-pipeline/src/jev_research_pipeline/jev/source_support.py']}
(R/'method-demo.json').write_text(json.dumps(method,indent=2)+'\n')
print(len(cases),'cases;',len(threads),'threads; six actual code runs')
