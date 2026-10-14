import urllib.request,json,pathlib,hashlib
R=pathlib.Path(__file__).parent/'sources';R.mkdir(exist_ok=True)
repos=['devagrawal09/stanley-code','shimo4228/jev-research-pipeline','kitze/skillbox','doeixd/discern']
def get(u):
 return urllib.request.urlopen(urllib.request.Request(u,headers={'User-Agent':'knowledge-garden-research'}),timeout=35).read()
out=[]
for repo in repos:
 try:
  meta=json.loads(get('https://api.github.com/repos/'+repo));sha=json.loads(get('https://api.github.com/repos/'+repo+'/commits/'+meta['default_branch']))['sha']; tree=json.loads(get('https://api.github.com/repos/'+repo+'/git/trees/'+sha+'?recursive=1'))
  paths=[x['path']for x in tree['tree']if x['type']=='blob'];out.append({'repo':repo,'sha':sha,'paths':paths});print(repo,sha);print('\n'.join(p for p in paths if any(w in p.lower()for w in ['jev','memory','skill','workflow','decision','proposal','research','procedure']) and not any(w in p for w in ['lock','.svg','fixtures','.png']))[:14000])
 except Exception as e:out.append({'repo':repo,'error':str(e)});print(repo,type(e).__name__,str(e))
(R/'trees.json').write_text(json.dumps(out,indent=2))
for name in ['api','primitives/noul','primitives/choice','concepts/state','patterns/composite-scoring','cookbooks/citation_check']:
 try:
  raw=get('https://docs.typesafe.ai/'+name+'.md');(R/('typesafe-'+name.replace('/','-')+'.md')).write_bytes(raw)
 except Exception as e:print(name,type(e).__name__)
