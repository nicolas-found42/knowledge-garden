"""Pre-register synthetic labels and questions before live requests.
No external repository implementation is imported or executed.
"""
import json
from pathlib import Path
OUT=Path(__file__).parent
cases=[]
def add(family,name,state,expected,baseline):
 cases.append({'family':family,'name':name,'state':state,'expected':expected,'baseline':baseline})
# Inspired by Byeori: lookup failure must not manufacture a durable wiki gap.
rows=[
 ('correction','Wiki cache expires after 24h.','User: We measured memory growth and now choose a 1h expiry for this wiki. Source is complete.','review'),
 ('covered','Wiki cache expires after 1h.','User: Keep the one-hour wiki cache expiry. Source is complete.','answer'),
 ('truncated','Wiki cache expiry policy is not in the supplied window.','Excerpt: We choose... [TRUNCATED]. The omitted text includes the actual policy.','lookup'),
 ('dropped','No relevant wiki passages supplied.','The retriever dropped the policy note because the byte budget was exceeded.','lookup'),
 ('speculation','Wiki uses a one-hour cache.','Assistant: Perhaps a five-minute cache would be faster. No measurement or user decision is supplied.','answer'),
 ('known_limitation','Known limitation: title-only dedup misses paraphrases.','A note observes title-only dedup missing paraphrases; this is already listed as a limitation.','answer'),
 ('new_failure','Dedup says title matches are always safe to merge.','Observed trace: notes titled Atlas refer to different projects; source IDs and contents prove a wrong merge. The general rule is unsafe.','review'),
 ('oneoff','Wiki default cache is one hour.','User: On my battery-starved train trip today disable cache indexing. The wiki default is unchanged.','answer'),
 ('unresolved','Wiki conflict behavior is not supplied.','User asks whether contradictory sources should replace notes. No cited source or current policy is supplied.','lookup'),
 ('same_meaning','Canonical notes are .md files; indexes are rebuildable.','User: Markdown is authoritative; SQLite is just a disposable index. The source is complete.','answer'),
]
for name,wiki,evidence,label in rows:
 st={'existing_wiki':wiki,'new_evidence':evidence,'purpose':'Decide whether this exchange exposes a concrete reusable maintenance issue, not whether its scientific claims are universally true.'}
 baseline='review' if any(x in evidence.lower() for x in ['now choose','wrong','miss','contradict','policy','no relevant']) else 'answer'
 add('maintenance',name,st,{'route':label},{'route':baseline})
# Selectively mark current derived assertions for refresh after a source edit.
changes=[
 ('database','The wiki uses SQLite.','The wiki now uses Postgres.','The wiki currently uses SQLite.', 'The wiki used SQLite in the 2025 prototype.'),
 ('retention','Cache expires in 24h.','Cache now expires in 1h.','Cache expires in 24h.', 'In the April experiment, expiry was 24h.'),
 ('condition','All notes may be sent to the provider.','Only public notes may be sent to the provider.','All notes may be sent to the provider.', 'Earlier, we experimented with sending all notes.'),
 ('mandatory','Evidence citations are optional.','Evidence citations are now mandatory.','Evidence citations are optional.', 'The initial draft allowed optional citations.'),
 ('reversed','Graph server is required.','Graph server is explicitly no longer required.','A graph server is required.', 'The first plan required a graph server.'),
 ('typo','Canonical notes are Markdown files.','Canonical notes are Markdown files. Typo in heading fixed.','Canonical notes are Markdown files.', 'The original source chose Markdown files.'),
 ('format','Use a one-hour cache.','Use a 1h cache. Same meaning; formatting only.','Use a one-hour cache.', 'We originally chose a one-hour cache.'),
 ('scope','Wiki uses SQLite. Payments uses Postgres.','Wiki still uses SQLite. Payments now uses MySQL.','Wiki uses SQLite.', 'Payments used Postgres before this edit.'),
 ('reason','SQLite chosen because measured speed was twice as fast.','SQLite chosen for simplicity; the speed comparison was invalid and retracted.','SQLite was measured twice as fast.', 'A speed claim was made in the original meeting.'),
 ('provenance','Trace T1 verified command A passed.','Trace T1 was found fabricated; there is no evidence that command A passed.','Command A is verified to pass.', 'An unverified report originally claimed A passed.'),
]
for name,old,new,current,historical in changes:
 exp='keep' if name in ('typo','format','scope') else 'refresh'
 notes=[{'id':'current','text':current,'kind':'current_fact','depends_on':'s1'}, {'id':'history','text':historical,'kind':'historical_record','depends_on':'s1'}, {'id':'separate','text':'User prefers dark mode.','kind':'current_fact','depends_on':'s1'}]
 add('impact',name,{'source_id':'s1','old_source':old,'new_source':new,'dependent_notes':notes},{'current':exp,'history':'keep','separate':'keep'},{'current':'refresh','history':'refresh','separate':'refresh'})
# Acquire the missing piece; relevant-looking repetition is a distractor.
context_rows=[
 ('pronoun','Did the user accept SQLite?','User: Yes, do that.','The assistant proposal immediately before this reply says: Use SQLite for the wiki index.','A SQLite tutorial describes indexes.','neighbor'),
 ('pronoun_other','Did the user accept SQLite?','User: Yes, do that.','The assistant proposal immediately before this reply says: Use Postgres for the wiki index.','A SQLite tutorial describes indexes.','neighbor'),
 ('test_claim','Is command A proven to work on Windows?','Assistant: All tests pass.','Original CI artifact lists operating system Windows and command A exit 0.','A README discusses testing command A on Windows.','artifact'),
 ('test_missing','Is command A proven to work on Windows?','Assistant: All tests pass.','Original CI artifact lists Linux only; no Windows run.','A README discusses testing command A on Windows.','artifact'),
 ('current_policy','What cache expiry is active now?','An old source says cache expiry is 24h.','Latest user decision says replace 24h cache with 1h.','A 24h cache benchmark exists.','policy'),
 ('exception','Is Python now the user standing preference?','User: Use Python for this one throwaway prototype.','Standing preferences note: default language TypeScript unless the user changes the general preference.','A Python article says prototypes are fast.','policy'),
 ('fabricated','Did the cited quote exist?','Proposed quote: Every graph edge is verified.','Complete original source text and quote offsets.','A summary repeats Every graph edge is verified.','artifact'),
 ('same_project','Which Atlas project did the decision refer to?','User: Put it in Atlas.','Earlier messages identify Atlas as the gardening notebook, distinct from payment service Atlas.','Documentation for payment service Atlas.','neighbor'),
 ('already_sufficient','Did the user accept SQLite?','User: Use SQLite for this wiki index; this is my final choice.','An unrelated old discussion about Postgres.','A SQLite tutorial describes indexes.','none'),
 ('correction','Was the first architecture abandoned?','User: Actually, revert that choice.','The preceding user decision chose Neo4j; this reply explicitly refers to that choice.','An architecture guide describes Neo4j.','neighbor'),
]
for name,query,frag,bridge,decoy,label in context_rows:
 target=label if label!='none' else 'neighbor'
 cs={target:bridge,'distractor':decoy,'none':'No more lookup is needed; current fragment itself answers the question.'}
 add('context',name,{'query':query,'fragment':frag,'available_fetches':cs},{'fetch':label},{'fetch':'distractor'})
# Candidate procedure is not automatically installed or universally true.
proc_rows=[
 ('repeated',[('s1','run tests then inspect failure logs','completed'),('s2','run tests then inspect failure logs','completed'),('s3','run tests then inspect failure logs','completed')],'candidate'),
 ('same_session',[('s1','run tests then inspect failure logs','completed')]*3,'none'),
 ('all_failed',[('s1','run tests then inspect failure logs','failed'),('s2','run tests then inspect failure logs','failed'),('s3','run tests then inspect failure logs','failed')],'evidence'),
 ('retries',[('s1','failed command then corrected it and ran tests successfully','completed'),('s2','ran corrected tests successfully','completed'),('s3','ran corrected tests successfully','completed')],'candidate'),
 ('abandoned',[('s1','run tests then inspect failure logs','completed'),('s2','run tests then inspect failure logs','completed'),('s3','run tests then inspect failure logs','completed'),('s4','User: abandon this procedure; it corrupts state','abandoned')],'none'),
 ('preference',[('s1','user prefers dark mode','completed'),('s2','user prefers dark mode','completed'),('s3','user prefers dark mode','completed')],'none'),
 ('oneoff',[('s1','apply ticket 421 patch only','completed'),('s2','apply ticket 421 patch only','completed'),('s3','apply ticket 421 patch only','completed')],'none'),
 ('unknown_flags',[('s1','journal says run tests then inspect logs; exact flags absent','completed'),('s2','journal says run tests then inspect logs; exact flags absent','completed'),('s3','journal says run tests then inspect logs; exact flags absent','completed')],'candidate'),
 ('scope',[('s1','Wiki project: rebuild index then verify note citations','completed'),('s2','Wiki project: rebuild index then verify note citations','completed'),('s3','Wiki project: rebuild index then verify note citations','completed')],'candidate'),
 ('unsafe_generalization',[('s1','in this demo only delete scratch index then rebuild','completed'),('s2','in this demo only delete scratch index then rebuild','completed'),('s3','production index has irreplaceable audit data; never delete it','warning')],'evidence'),
]
for name,eps,label in proc_rows:
 st={'episodes':[{'session_id':s,'text':t,'outcome':o} for s,t,o in eps], 'candidate_policy':'A draft candidate needs a repeatable multi-step operation in at least 3 distinct sessions, with completed outcomes and no later abandonment. Facts/preferences and one-ticket repeats are not procedures. Keep project scope and unknown details visible; do not install anything.'}
 baseline='candidate' if len(eps)>=3 else 'none'
 add('procedure',name,st,{'route':label},{'route':baseline})
(OUT/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
print('Pre-registered',len(cases),'cases')
