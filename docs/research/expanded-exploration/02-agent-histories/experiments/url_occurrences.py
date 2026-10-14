"""Capture HTTP(S) occurrences without fetching. No HTTP client imports.

Conservative identity lowercases scheme/host and removes only default ports;
path, query order, tracking parameters, percent escapes and fragments survive.
Domain means exact hostname, not inferred registrable domain. Original substrings
and offsets always survive independently of normalization.
"""
import argparse, collections, hashlib, json, re
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit
PATTERN=re.compile(r'https?://[^\s<>"\x00-\x1f]+',re.I)
def normalize(original):
    try:
        p=urlsplit(original);host=p.hostname
        if not host:return None,None
        # Preserve userinfo; never expose it in public records.
        user=p.netloc.rsplit('@',1)[0]+'@' if '@' in p.netloc else ''
        h=host.lower();h='['+h+']' if ':' in h else h
        port=p.port
        authority=user+h+(':'+str(port) if port is not None and (p.scheme.lower(),port) not in [('https',443),('http',80)] else '')
        identity=urlunsplit((p.scheme.lower(),authority,p.path,p.query,p.fragment))
        # urlunsplit removes empty delimiters; conservatively retain them.
        if '?' in original.split('#',1)[0] and not p.query:
            base,sep,frag=identity.partition('#');identity=base+'?'+(sep+frag if sep else '')
        if '#' in original and not p.fragment:identity+='#'
        return identity,host.lower()
    except ValueError:return None,None
def occurrences(value,path=()):
    if isinstance(value,str):
        for m in PATTERN.finditer(value):
            # Only a known Markdown destination delimiter can be removed.
            # Bare trailing punctuation is ambiguous and may be meaningful path
            # data; keep it rather than falsely merging two destinations.
            raw=m.group();end=m.end()
            while value[:m.start()].endswith('](') and raw.endswith(')') and raw.count(')')>raw.count('('):
                raw=raw[:-1];end-=1
            ident,host=normalize(raw)
            yield dict(field=list(path),start=m.start(),end=end,original=raw,lexical_match=m.group(),lexical_end=m.end(),delimiter_trimmed=raw!=m.group(),identity=ident,domain=host,status='merely_linked',destination_fetched=False)
    elif isinstance(value,dict):
        for k,v in value.items():yield from occurrences(v,path+(k,))
    elif isinstance(value,list):
        for i,v in enumerate(value):yield from occurrences(v,path+(i,))
def main():
    p=argparse.ArgumentParser();p.add_argument('--private-dir',type=Path,required=True);args=p.parse_args();dst=args.private_dir.resolve()
    if dst.is_relative_to(Path(__file__).resolve().parents[5]):raise SystemExit('Private evidence directory must be outside repository')
    ss=json.loads((dst/'sources.json').read_text());out=[]
    for s in ss:
        if s['app']=='Hermes':
            records=[(m['id'],m) for m in s['messages']]
        else:
            records=[]
            for i,line in enumerate(Path(s['path']).open(),1):
                try:records.append((f'L{i}',json.loads(line)))
                except ValueError:pass
        for rid,value in records:
            for o in occurrences(value):out.append(dict(occurrence_id=f'occ-{len(out):06d}',thread=s['id'],record=rid,**o))
    (dst/'url-occurrences.json').write_text(json.dumps(out,indent=2))
    groups=collections.defaultdict(set);domains=collections.defaultdict(set)
    for o in out:
        if o['identity'] is not None:groups[o['identity']].add(o['thread'])
        if o['domain'] is not None:domains[o['domain']].add(o['thread'])
    # Explicit adjacency, not merely aggregate counts. Invalid identities remain
    # independent occurrences and cannot create a spurious null identity edge.
    adjacency=dict(identities={k:[o['occurrence_id'] for o in out if o['identity']==k] for k in groups},domains={k:[o['occurrence_id'] for o in out if o['domain']==k] for k in domains})
    (dst/'url-adjacency.json').write_text(json.dumps(adjacency,indent=2))
    summary=dict(threads=len(ss),occurrences=len(out),identities=len(groups),domains=len(domains),identities_in_multiple_threads=sum(len(v)>1 for v in groups.values()),domains_in_multiple_threads=sum(len(v)>1 for v in domains.values()),invalid=sum(o['identity'] is None for o in out),destination_fetch_attempts=0,status_counts=dict(collections.Counter(o['status'] for o in out)),scope='All string fields in selected JSONL records; Hermes selected extracted message fields. Not whole available corpus.')
    (dst/'url-summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
