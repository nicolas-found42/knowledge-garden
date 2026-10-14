"""Offline artifact consistency, not production tests or human validation."""
from pathlib import Path
import ast,json,re,urllib.parse,hashlib,subprocess,datetime
R=Path(__file__).resolve().parent;ROOT=next(p for p in R.parents if (p/'AGENTS.md').exists());fail=[];counts={}
files=list(R.rglob('*'));js=[]
for p in files:
    if not p.is_file():continue
    if p.suffix=='.json':
        try:json.loads(p.read_text());counts['jsonParsed']=counts.get('jsonParsed',0)+1
        except Exception as e:fail.append({'file':str(p.relative_to(R)),'error':str(e)})
    if p.suffix=='.py':
        try:ast.parse(p.read_text());counts['pythonParsed']=counts.get('pythonParsed',0)+1
        except Exception as e:fail.append({'file':str(p.relative_to(R)),'error':str(e)})
    if p.suffix in ['.js','.cjs','.mjs']:js.append(p)
    if p.suffix=='.md' and ('experiments' not in p.parts or p.name=='README.md'):
        # Check our reports/indexes, not third-party snapshots or intentional link-break fixtures.
        s=p.read_text();links=re.findall(r'\]\(([^)]+)\)',s)+re.findall(r'^\[[^]]+\]:\s+(\S+)',s,re.M)
        for link in links:
            link=link.strip('<>');path=urllib.parse.unquote(link.split('#')[0].split('?')[0])
            if not path or urllib.parse.urlsplit(path).scheme:continue
            if not (p.parent/path).exists():fail.append({'file':str(p.relative_to(R)),'missingLink':path})
            counts['localLinksChecked']=counts.get('localLinksChecked',0)+1
for p in js:
    x=subprocess.run(['node','--check',str(p)],capture_output=True,text=True);counts['javascriptChecked']=counts.get('javascriptChecked',0)+1
    if x.returncode:fail.append({'file':str(p.relative_to(R)),'error':x.stderr})
E=R/'04-search-and-timelines/experiments';D=json.loads((E/'corpus.json').read_text());ids={x['id'] for x in D['items']}
for x in D['items']:
    if x['sourceHref'] and not (ROOT/x['sourceHref'].lstrip('/')).exists():fail.append({'missingSource':x['id']})
for x in D['relations']:
    if x['from'] not in ids or x['to'] not in ids or any(e['itemId'] not in ids for e in x['evidence']):fail.append({'badRelationship':x['id']})
counts['corpusItems']=len(ids);counts['relations']=len(D['relations'])
for x in json.loads((E/'source-manifest.json').read_text())['files']:
    if hashlib.sha256((E/x['file']).read_bytes()).hexdigest()!=x['sha256']:fail.append({'assetHashMismatch':x['file']})
counts['assetHashes']=len(json.loads((E/'source-manifest.json').read_text())['files'])
if hashlib.sha256((E/'corpus.json').read_bytes()).hexdigest()!=json.loads((E/'frozen-labels.json').read_text())['corpus_sha256']:fail.append({'corpus':'frozen hash mismatch'})
summary=json.loads((E/'run-20260930T004745Z/summary.json').read_text());usage=json.loads((R/'usage.json').read_text())
if abs(sum(x['reportedCostUSD'] for x in summary['metrics'].values())-usage['reportedRouterCostUSD'])>1e-12:fail.append({'usage':'cost mismatch'})
result=dict(recordedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),checks=counts,failures=fail,status='pass' if not fail else 'needs-repair',actualBrowser='03-interface-concepts/browser-results.md',assertionEvidence={'durableMarkdown':'26/26 retained original run','macWatcher':'7/7 retained strengthened run','timeline':'9/9','publicAssets':'8/8'},notEstablished=['actual Obsidian interoperability','human usability','WCAG conformance','production native runtime','end-to-end large-collection navigation','atomic multiwriter safety','general extraction/model accuracy'])
(R/'validation.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
