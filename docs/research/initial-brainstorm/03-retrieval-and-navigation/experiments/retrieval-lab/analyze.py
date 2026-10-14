"""Offline analysis: never invokes API; preserve raw labels and separate candidate coverage."""
import collections,json,statistics
from run import ROOT,QUERIES,CORPUS,lexical
initial=json.loads((ROOT/'results-initial.json').read_text());follow=json.loads((ROOT/'results-followup.json').read_text());facets=json.loads((ROOT/'results-facets.json').read_text());rows=initial['results']+follow['results']+facets['results'];ok=[r for r in rows if 'response'in r]
by=collections.defaultdict(list)
for r in rows:by[r['family']].append(r)
families={}
for family,rs in by.items():
 checks=[c for r in rs for c in r.get('checks',{}).values()]
 families[family]={'api_calls':len(rs),'questions':sum(len(r['request']['questions'])for r in rs),'raw_label_checks':len(checks),'raw_correct':sum(c['correct']for c in checks)}
retrieval=[]
for q in QUERIES:
 ir=[r for r in by['independent']if r['name'].startswith(q['id']+'__')]
 score_rank=sorted(ir,key=lambda r:-r['response']['answers']['directness']['score'])
 noul_rank=sorted(ir,key=lambda r:-r['response']['answers']['helps']['noul'])
 gold=q['best'];top=lexical(q['query'])[:3]
 entry={'query':q['id'],'gold':gold,'lexical_top1':lexical(q['query'])[0],'lexical_top3':top,'candidate_gold_coverage':bool(set(gold)&set(top))if gold else None,'independent_score_top1':score_rank[0]['name'].split('__')[1],'independent_noul_top1':noul_rank[0]['name'].split('__')[1],'variants':{}}
 for mode in ['broad','lexical_top3']:
  r=next(r for r in by['recall_choice']if r['name']==q['id']+'_'+mode);supplied_gold=bool(gold)and(mode=='broad'or bool(set(gold)&set(top)))
  a=r['response']['answers'];entry['variants'][mode]={'best':a['best']['choice'],'corpus_selection_correct':a['best']['choice']in(gold or ['none']),'answerability_noul':a['answerable']['noul'],'supplied_state_answerability_gold':supplied_gold,'supplied_state_answerability_correct':(a['answerable']['noul']>=.5)==supplied_gold,'failure_attribution':'candidate_recall'if gold and mode=='lexical_top3'and not set(gold)&set(top)else 'ranking_or_abstention'if a['best']['choice']not in(gold or ['none'])else None}
 retrieval.append(entry)
for g in follow['graphs']:
 for completion in g['completed']:
  required={'trace':{'revisions','mechanics'},'search':{'choice'},'absent':{'no_such_node'}}[g['name']]
  completion['verified_required_evidence_coverage']=required.issubset(completion['path'])
  completion['premature_completion']=not completion['verified_required_evidence_coverage']
axes={r['name']:{k:a['score']/2 for k,a in r['response']['answers'].items()}for r in by['cache_axes']}
weights={'implementation':{'constraint':.1,'procedure':.8,'provenance':.1},'decision_audit':{'constraint':.1,'procedure':.1,'provenance':.8},'planning':{'constraint':.8,'procedure':.1,'provenance':.1}}
cache_views={name:sorted([{'source':sid,'weighted_score':sum(axes[sid][k]*w for k,w in ws.items())}for sid in axes],key=lambda r:-r['weighted_score'])[:5]for name,ws in weights.items()}
failures=[{'family':r['family'],'name':r['name'],'question':k,**v}for r in ok for k,v in r.get('checks',{}).items()if not v['correct']]
s={
 'study_status':'Synthetic author-labeled development probes. Initial labels frozen before API calls; follow-up wording/composition informed by observed failures. No untouched holdout.',
 'usage':{'api_calls':len(rows),'successful':len(ok),'questions':sum(len(r['request']['questions'])for r in rows),'input_tokens':sum(r['response']['usage']['input_tokens']for r in ok),'output_tokens':sum(r['response']['usage']['output_tokens']for r in ok),'reported_cost_usd':sum(r['response']['usage']['cost']for r in ok),'median_request_seconds':statistics.median(r['seconds']for r in ok),'resolved_models':sorted(set(r['response']['model']for r in ok))},
 'fixture_counts':{'corpus_sources':len(CORPUS),'retrieval_queries':8,'query_source_pairs':160,'bundle_states':6,'temporal_queries':4,'clarification_states':4,'graph_goals':3,'axes_sources':20,'initial_distractor_pool_sizes':4,'followup_equivalent_pool_sizes':4,'note':'Shared/repeated fixtures overlap; these are not independent sample counts and should not be summed.'},
 'families':families,'retrieval':retrieval,'retrieval_summary':{'answerable_queries':6,'lexical_top1_correct':sum(r['lexical_top1']in r['gold']for r in retrieval if r['gold']),'lexical_top3_gold_coverage':sum(r['candidate_gold_coverage']is True for r in retrieval),'broad_choice_correct_all8':sum(r['variants']['broad']['corpus_selection_correct']for r in retrieval),'shortlist_choice_correct_all8':sum(r['variants']['lexical_top3']['corpus_selection_correct']for r in retrieval),'independent_score_top1_correct_answerable6':sum(r['independent_score_top1']in r['gold']for r in retrieval if r['gold']),'independent_noul_top1_correct_answerable6':sum(r['independent_noul_top1']in r['gold']for r in retrieval if r['gold']),'supplied_state_answerability_correct16':sum(v['supplied_state_answerability_correct']for r in retrieval for v in r['variants'].values())},
 'facet_graph':{k:v for k,v in facets.items() if k!='results'},'initial_graph':initial['graph_paths'],'followup_graph':follow['graphs'],'cached_axes':axes,'preference_weights':weights,'cached_views':cache_views,'raw_label_failures':failures,'service_errors':[{'family':r['family'],'name':r['name'],'error_type':r['error_type']}for r in rows if 'error_type'in r]}
(ROOT/'summary.json').write_text(json.dumps(s,indent=2)+'\n')
print(json.dumps({k:s[k]for k in ['usage','families','retrieval_summary','cached_views','followup_graph']},indent=2))
