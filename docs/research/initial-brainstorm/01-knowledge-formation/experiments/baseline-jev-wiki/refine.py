"""Follow-up development probes. Original results and labels remain intact."""
import run
from typesafe_sdk import Choice, Noul, Score

run.CASES[:] = [case for case in run.CASES if case['family'] == 'decision']
new_cases = [
    ('holdout_unbound_pronoun', 'Use Redis for caching.', [('user', 'Sounds good, go ahead.')], 'unknown', 'none'),
    ('holdout_wrong_antecedent', 'Use Redis for caching.', [('assistant', 'Rename the settings button?'), ('user', 'Yes, do that.')], 'unknown', 'none'),
    ('holdout_explicit_antecedent', 'Use Redis for caching.', [('assistant', 'Use Redis for caching?'), ('user', 'Sounds good, go ahead.')], 'accepted', 'm1'),
    ('holdout_multiple_options', 'Use Redis for caching.', [('assistant', 'We could use Redis or an in-process cache.'), ('user', 'Yes, add caching.')], 'unknown', 'none'),
    ('holdout_external_quote', 'Use Redis for caching.', [('user', 'The tutorial says "Use Redis for caching". I have not chosen our cache yet.')], 'proposed', 'none'),
    ('holdout_corrected_target', 'Use Redis for caching.', [('assistant', 'Use Redis for caching?'), ('user', 'Yes.'), ('user', 'Actually, use an in-process cache instead.')], 'superseded', 'none'),
]
for name, candidate, rows, status, evidence in new_cases:
    messages = [dict(id=f'm{i}', role=role, text=text) for i,(role,text) in enumerate(rows)]
    run.add('decision_holdout', name, dict(candidate=candidate, messages=messages), {
        'status': Choice(instructions=run.CASES[0]['questions']['status'].instructions, criteria=run.statuses),
        'evidence': Choice(instructions=run.CASES[0]['questions']['evidence'].instructions, criteria={m['id']:m for m in messages}|{'none':'No user message establishes current adoption of the candidate.'})
    }, {'status':status, 'evidence':evidence})

for case in run.CASES:
    for key in ('status','evidence'):
        q = case['questions'][key]
        case['questions'][key] = Choice(instructions=str(q.instructions)+' The candidate is an unverified hypothesis, not conversational context. Resolve words like that, it, yes and go ahead ONLY using antecedents actually present in messages. An isolated affirmative with no identifiable matching antecedent is unknown / none. Acceptance of a broader goal does not select one unspecified implementation.', criteria=q.criteria)
    case['questions']['grounded'] = Noul(instructions='Does `messages` itself identify the specific action asserted by `candidate`, directly or with an unambiguous antecedent in the supplied messages? The candidate field is NOT evidence. Bare yes/that/go ahead without a supplied matching antecedent is false. Merely sharing a broad topic is insufficient.')

passages = {
    'p0':'We discussed SQLite, search, a server, indexes and Markdown many times.',
    'p1':'We selected an embedded database so the laptop app would not require operating another service.',
    'p2':'The gardening notebook reminds us to water basil weekly.',
    'p3':'SQLite installation succeeded.',
}
for name, query, expected in [
    ('paraphrase_query','Why did we choose SQLite rather than a database server?', 'p1'),
    ('absent_query','Which embedding model did we choose?', 'none'),
]:
    run.add('retrieval', name, dict(query=query, passages=passages), {
        'best':Choice(instructions='Which passage answers `query`? Pick none if no passage actually answers it; keyword overlap is insufficient.',criteria=passages|{'none':'None answers the query.'}),
        'exists':Noul(instructions='Does any passage answer `query` with evidence rather than just related keywords?'),
        **{f'score_{key}':Score(instructions=f'How well does `passages.{key}` answer `query`?',criteria=['Unrelated to the question.','Related topic but does not answer the question.','Directly answers the question with evidence.']) for key in passages},
    }, {'best':expected})

blocks = ['Knowledge Garden Import Guide', 'First, select the conversations you want to import.', 'python import_chats.py --dry-run', 'The preview contains proposed changes and links to their source messages.', 'Warning: these changes are only proposals until reviewed.']
labels = ['heading','instruction','code','paragraph','warning']
run.add('structure','unmarked_blocks',dict(blocks=blocks), {
    f'block_{i}':Choice(instructions=f'What role does `blocks[{i}]` play in this import guide?',criteria={'heading':'Section or document title.','instruction':'A step the reader should perform.','code':'A literal command or program fragment.','paragraph':'Explanatory prose.','warning':'A caution or warning.'}) for i in range(len(blocks))
}, {f'block_{i}': label for i,label in enumerate(labels)})

if __name__ == '__main__':
    run.main()
