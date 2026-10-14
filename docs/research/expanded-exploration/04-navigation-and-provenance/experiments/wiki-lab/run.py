"""Six frozen generation inputs, then six original-evidence verification requests. No retries."""
import json,pathlib,hashlib,os,time,urllib.request,datetime
R=pathlib.Path(__file__).parent
F=R/'portable-fixtures.json';data=json.loads(F.read_text());by={b['id']:b for b in data['bundles']}
def api(endpoint,body):
 start=time.perf_counter();req=urllib.request.Request('https://openrouter.ai/api/v1/'+endpoint,data=json.dumps(body).encode(),headers={'Authorization':'Bearer '+os.environ['OPENROUTER_API_KEY'],'Content-Type':'application/json'})
 try:
  with urllib.request.urlopen(req,timeout=100)as response:raw=json.load(response)
  return {'request':body,'response':raw,'seconds':time.perf_counter()-start}
 except Exception as e:return {'request':body,'error_type':type(e).__name__,'error_message':str(e),'seconds':time.perf_counter()-start}
results=[]
for v in data['variants']:
 prompt={'instruction':'Write a readable source-linked wiki capsule as JSON {title,sentences:[{text,source_ids,status}]}. At most FOUR short sentences. Status must be recorded/tentative/speculative. Refer only to supplied source IDs. Preserve constructed/authored origins when supplied; do not infer real personal history from an observation. Preserve bounded claims, uncertainty, source conversions and relevant exceptions. Distinguish measured outputs from assistant assertions and hypotheses from facts. If metadata or evidence is missing, leave claims tentative or explicitly unknown. No external knowledge, invented outcomes, causal improvement claims, or skill activation. Cite the particular records that support each sentence. Make this understandable to someone without the experiment context.','bundle':v['bundle'],'input':v['generation_input']}
 g=api('chat/completions',{'model':'openai/gpt-4.1-mini','temperature':0,'response_format':{'type':'json_object'},'messages':[{'role':'user','content':json.dumps(prompt)}]});g.update(id=v['id'],engine='generative',variant=v['variant']);results.append(g)
 if 'response'not in g:continue
 try:
  capsule=json.loads(g['response']['choices'][0]['message']['content']);g['parsed_capsule']=capsule
  originals={s['id']:s for s in by[v['bundle']]['sources']};claims=[]
  for i,sentence in enumerate(capsule['sentences']):claims.append({'index':i,'generated_sentence':sentence,'original_referenced_sources':[originals[id]for id in sentence['source_ids']if id in originals],'unknown_source_ids':[id for id in sentence['source_ids']if id not in originals]})
  qs={}
  for c in claims:
   i=c['index'];qs['fidelity_'+str(i)]={'type':'choice','instructions':f'Does claims[{i}].generated_sentence.text faithfully represent ONLY original_referenced_sources? Quote occurrence is not adoption; assistant claims are not measured output. Reject invented history, unsupported before/after improvement, universal capability and speculation presented as recorded. Inspect origin as well as content.','criteria':{'faithful':'Sentence supported with essential scope/origin preserved.','qualifier_missing':'Core content supported but an essential provenance/scope/uncertainty qualifier omitted.','unsupported':'Sentence adds a claim its original sources do not establish, contradicts them or cites unavailable IDs.'}};qs['status_'+str(i)]={'type':'noul','instructions':f'Is claims[{i}].generated_sentence.status appropriate given original source provenance, modality and claim content? A recorded observation within an explicitly constructed history may be recorded; analogy/application transfer is speculative.'}
  j=api('systemone',{'model':'typesafe/jev-1.13','state':{'claims':claims,'bundle_original_provenance':'Public source excerpts and explicitly constructed controlled histories; no real private-user history in this experiment.'},'questions':qs});j.update(id=v['id']+'-verify',engine='jev',variant=v['variant']);results.append(j)
 except Exception as e:g['parse_or_control_error']={'type':type(e).__name__,'message':str(e)}
 print('finished',v['id'],flush=True)
(R/'results.json').write_text(json.dumps({'recorded_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'fixture_sha256':hashlib.sha256(F.read_bytes()).hexdigest(),'results':results},indent=2)+'\n');print('requests',len(results))
