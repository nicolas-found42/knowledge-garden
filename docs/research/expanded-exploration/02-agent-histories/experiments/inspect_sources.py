"""Read-only local history inventory. All source mappings/content written privately.

Does not read credentials/config values and has no network code. Output directory
must be outside the repository. Selection is metadata-stratified, not random.
"""
import argparse, collections, hashlib, json, os, sqlite3
from pathlib import Path

def opaque(s): return hashlib.sha256(str(s).encode()).hexdigest()[:16]
def text(content):
    if isinstance(content,str):return content
    if isinstance(content,list):return '\n'.join(x.get('text','') for x in content if isinstance(x,dict) and x.get('type') in ['text','input_text','output_text'])
    return ''
def parse(path,app):
    out=[];types=collections.Counter();title='';schema=set()
    for line_no,line in enumerate(path.open(),1):
        try:d=json.loads(line)
        except ValueError:continue
        typ=d.get('type','unknown');types[typ]+=1;schema.update(d)
        if app=='Codex':
            p=d.get('payload',{})
            if typ=='response_item' and p.get('type')=='message':
                out.append(dict(id=f'L{line_no}',role=p.get('role'),text=text(p.get('content'))))
            elif typ=='response_item' and p.get('type') in ['function_call','function_call_output','custom_tool_call','custom_tool_call_output']:
                out.append(dict(id=f'L{line_no}',role='tool',text=p.get('output',p.get('arguments',p.get('input',''))),kind=p.get('type')))
        elif app=='Claude':
            if typ=='custom-title':title=d.get('customTitle','')
            if typ in ['user','assistant']:
                m=d.get('message',{});c=m.get('content','')
                out.append(dict(id=d.get('uuid',f'L{line_no}'),parent=d.get('parentUuid'),role=m.get('role',typ),text=text(c)))
                if isinstance(c,list):
                    for k,b in enumerate(c):
                        if b.get('type') in ['tool_use','tool_result']:
                            out.append(dict(id=f'L{line_no}B{k}',role='tool',text=json.dumps(b),kind=b['type']))
        else:
            if typ=='title':title=d.get('title','')
            if typ=='message':
                m=d.get('message',{});out.append(dict(id=d.get('id',f'L{line_no}'),parent=d.get('parentId'),role=m.get('role'),text=text(m.get('content')),kind=typ))
                for k,b in enumerate(m.get('content',[]) if isinstance(m.get('content'),list) else []):
                    if isinstance(b,dict) and b.get('type')=='toolCall':
                        out.append(dict(id=f'L{line_no}B{k}',role='tool',text=json.dumps(b),kind='toolCall',parent=d.get('id')))
    return dict(messages=out,title=title,record_types=dict(types),schema=sorted(schema))
def main():
    p=argparse.ArgumentParser();p.add_argument('--private-dir',type=Path,required=True);args=p.parse_args()
    dst=args.private_dir.resolve();repo=Path(__file__).resolve().parents[5]
    if dst.is_relative_to(repo):raise SystemExit('Private directory must be outside repository')
    dst.mkdir(parents=True,exist_ok=True);os.chmod(dst,0o700)
    inventory={};sources=[]
    for app,rel in [('Codex','.codex/sessions'),('Claude','.claude/projects'),('OMP','.omp/agent/sessions')]:
        fs=sorted((Path.home()/rel).rglob('*.jsonl'),key=lambda x:(x.stat().st_size,str(x)))
        inventory[app]=dict(files=len(fs),bytes=sum(f.stat().st_size for f in fs),empty_files=sum(f.stat().st_size==0 for f in fs))
        # Exact size ranks across short/mid/long; bounded source reads.
        ranks=sorted(set([0,1,2,3,4,5,6,7]+[int((len(fs)-1)*q) for q in [.1,.25,.5,.75,.9]]))
        for i in ranks:
            f=fs[i];obj=parse(f,app);sid=app.lower()+'-'+opaque(f)
            sources.append(dict(id=sid,app=app,path=str(f),size=f.stat().st_size,rank=i,**obj))
    dbp=Path.home()/'.hermes/state.db';db=sqlite3.connect(f'file:{dbp}?mode=ro',uri=True)
    inventory['Hermes']=dict(sessions=db.execute('select count(*) from sessions').fetchone()[0],messages=db.execute('select count(*) from messages').fetchone()[0],role_counts=dict(db.execute('select role,count(*) from messages group by role')))
    ss=list(db.execute('select id,title,message_count from sessions order by message_count,id'))
    ranks=sorted(set([0,1,2,3,4,5,6,7]+[int((len(ss)-1)*q) for q in [.1,.25,.5,.75,.9]]))
    for i in ranks:
        session,title,n=ss[i];messages=[dict(id=str(r[0]),role=r[1],text=r[2] or '',tool_calls=r[3],active=r[4],compacted=r[5],compressed_summary=r[6]) for r in db.execute('select id,role,content,tool_calls,active,compacted,_compressed_summary from messages where session_id=? order by id',(session,))]
        sources.append(dict(id='hermes-'+opaque(session),app='Hermes',path=str(dbp),session_id=session,title=title,messages=messages,rank=i))
    (dst/'inventory.json').write_text(json.dumps(inventory,indent=2))
    (dst/'sources.json').write_text(json.dumps(sources,indent=2))
    summary=[dict(id=s['id'],app=s['app'],records=len(s['messages']),roles=dict(collections.Counter(m['role'] for m in s['messages'])),text_chars=sum(len(m['text']) for m in s['messages'] if isinstance(m['text'],str)),rank=s['rank']) for s in sources]
    (dst/'sample-metadata.json').write_text(json.dumps(summary,indent=2))
    print(json.dumps(dict(inventory=inventory,selected=len(sources),samples=summary),indent=2))
if __name__=='__main__':main()
