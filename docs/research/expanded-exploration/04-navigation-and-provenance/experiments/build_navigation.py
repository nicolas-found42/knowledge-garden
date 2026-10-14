"""Build a local evidence browser and deterministic navigation comparisons.

All inputs are saved public/constructed artifacts. This script never fetches URLs,
reads private source mappings, calls a model, or changes imported material.
"""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).parent
COLLECTION = HERE.parents[1]
MEDIA = COLLECTION / '01-source-interpretation/experiments/media-lab'
METHOD = COLLECTION / '03-connections-and-methods/experiments'
REL_MEDIA = '../../../01-source-interpretation/experiments/media-lab/'
REL_METHOD = '../../../03-connections-and-methods/experiments/'
OUT = HERE / 'navigation-lab'
DATE = '2026-09-29'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build():
    nodes = []

    def add(id, title, project, format, person, topic, text, *, status='recorded',
            origin='public source', file=None, relative=None, image=None, video=None,
            anchor=None, event=None, acquisition='captured', source_date=None):
        node = dict(id=id, title=title, project=project, format=format, person=person,
                    topic=topic, text=text, status=status, origin=origin,
                    acquisition=acquisition, observed_date=DATE, source_date=source_date,
                    anchor=anchor, event=event)
        if file:
            path = file[0] / file[1]
            if not path.exists():
                raise FileNotFoundError(path)
            node.update(href=relative + file[1], sha256=sha(path))
        if image:
            node['image'] = image
        if video:
            node['video'] = video
        nodes.append(node)

    def media(id, title, fmt, text, name, **kwargs):
        add(id, title, 'Motion experiment', fmt, 'Controlled researcher', 'Pixel changes', text,
            file=(MEDIA, name), relative=REL_MEDIA, **kwargs)

    media('motion-thread', 'Why the detector changed', 'JSON thread',
          'Four authored turns: request pixel-change evidence; propose grayscale and blur; explicitly adopt threshold 35; limit the interpretation to pixel changes.',
          'controlled-coding-thread.json', origin='constructed conversation', anchor='turns c1–c4', event={'order': 1, 'kind': 'proposal and adoption'})
    media('motion-before', 'Previous detector', 'Python',
          'Before-version script. Its status as a prior attempt belongs to the controlled demonstration, not a recovered personal project.',
          'controlled_demo.before.py', origin='constructed code', event={'order': 0, 'kind': 'prior implementation'})
    media('motion-after', 'Changed detector', 'Python',
          'Executed detector script. The change uses grayscale, blur and a threshold of 35; frame times remain available.',
          'controlled_demo.py', origin='executed constructed code', event={'order': 2, 'kind': 'implementation'})
    media('motion-metrics', 'Measured changed pixels', 'JSON measurements',
          'Actual per-frame measurements from the controlled run. Pixel counts do not establish object-tracking accuracy or suspicious behavior.',
          'controlled-demo-metrics.json', origin='measured output', anchor='50 timed records', event={'order': 3, 'kind': 'measurement'})
    media('motion-image', 'Rendered frame at 2 seconds', 'PNG',
          'Actual output screenshot of the controlled detector; source time 2.0 seconds.',
          'controlled-demo-screenshot.png', origin='rendered output', image=REL_MEDIA+'controlled-demo-screenshot.png',
          anchor='source video t=2.0 s', event={'order': 4, 'kind': 'demonstration'})
    media('motion-video', 'Demonstration video', 'MP4',
          'Actual rendered detector output based on the separately acquired public OpenCV vtest sample.',
          'controlled-demo.mp4', origin='rendered output', video=REL_MEDIA+'controlled-demo.mp4', anchor='source interval 0–10 s')
    add('motion-source', 'Original OpenCV video manifest', 'Motion experiment', 'AVI source record', 'OpenCV', 'Pixel changes',
        'Pinned vtest.avi source; the manifest identifies the acquired original binary. Its original recording date is unknown here.',
        file=(MEDIA, 'sources.json'), relative=REL_MEDIA, anchor='vtest.avi / commit d21a05f1…')

    add('book-page', 'Natural selection: printed page 78', 'Selection and diversity', 'Scanned PDF / PNG', 'Charles Darwin', 'Selection',
        'Real 1860 scan: variation and selection depend on conditions. PDF page 84 corresponds to printed page 78; they are different coordinate systems.',
        file=(MEDIA, 'book-pdf-page-84.png'), relative=REL_MEDIA, image=REL_MEDIA+'book-pdf-page-84.png', anchor='PDF page 84; printed page 78', source_date='1860')
    add('book-ocr', 'OCR linked to the page', 'Selection and diversity', 'OCR JSON', 'Local OCR engine', 'Selection',
        'Derived OCR from the scan, with region observations. The scan remains the primary evidence.',
        file=(MEDIA, 'book-pdf-page-84-ocr.json'), relative=REL_MEDIA, origin='derived OCR', anchor='PDF page 84')
    add('selection-article', 'DEAP author tutorial', 'Selection and diversity', 'RST article', 'DEAP maintainers', 'Selection',
        'The public tutorial describes operator behavior and warns that selection returns references rather than copied individuals.',
        file=(MEDIA, 'article-selection.rst'), relative=REL_MEDIA, anchor='Selection section; pinned source')
    add('selection-code', 'Selection implementations', 'Selection and diversity', 'Python', 'DEAP maintainers', 'Selection',
        'Pinned implementation from an established public project: several selection operators with different behavior. This is not one of the user’s recovered old projects.',
        file=(MEDIA, 'old-project-selection.py'), relative=REL_MEDIA, anchor='commit 8a96fd3a… / selection.py')
    add('selection-note', 'A question about retaining alternatives', 'Selection and diversity', 'Markdown note', 'Controlled researcher', 'Diversity',
        (MEDIA/'controlled-personal-note.md').read_text().strip(), file=(MEDIA, 'controlled-personal-note.md'), relative=REL_MEDIA,
        origin='constructed personal note', status='tentative', event={'order': 1, 'kind': 'unresolved question'})
    add('selection-analogy', 'Could diverse candidates help adaptation?', 'Selection and diversity', 'Connection proposal', 'Research synthesis', 'Diversity',
        'Speculative connection among the book, selection operators and the controlled note. A changing-workload experiment could test whether retaining alternatives improves later outcomes. No personal influence or historical adoption is established.',
        status='speculative', origin='authored hypothesis', file=(MEDIA, 'controlled-personal-note.md'), relative=REL_MEDIA,
        event={'order': 2, 'kind': 'hypothesis'})
    add('selection-failed-link', 'Rendered tutorial acquisition failed', 'Selection and diversity', 'URL record', 'DEAP maintainers', 'Selection',
        'A separately chosen public documentation page returned HTTP 403. The captured RST source is an alternative acquisition. This failed fetch is distinct from an intentionally unfetched conversation URL.',
        acquisition='could_not_retrieve', file=(MEDIA, 'sources.json'), relative=REL_MEDIA)

    demo = json.loads((METHOD/'method-demo.json').read_text())
    for i, thread in enumerate(demo['threads']):
        project = thread['project']
        add(thread['id'], {'csv_ids': 'Preserve leading-zero IDs', 'url_keys': 'Preserve URL query identity',
                          'cache_scope': 'Preserve tenant and policy scope', 'signed_urls': 'Signed URL sorting remains unresolved'}[project],
            'Methods across projects', 'JSON thread', 'Controlled researcher', 'Identity boundaries',
            thread['messages'][-1]['text'], origin=thread['status'], status='tentative' if project == 'signed_urls' else 'recorded',
            file=(METHOD, 'method-demo.json'), relative=REL_METHOD, anchor=thread['id'],
            event={'order': i, 'order_kind': 'source_group', 'kind': 'unresolved exception' if project == 'signed_urls' else 'failure then correction within this thread'})
    add('method-proposal', 'A reusable method with exceptions', 'Methods across projects', 'Method proposal', 'Research synthesis', 'Identity boundaries',
        demo['candidates']['preserve_boundary'] + ' Exceptions: ' + ' '.join(demo['exceptions']),
        file=(METHOD, 'method-demo.json'), relative=REL_METHOD, status='tentative', origin='authored method candidate',
        event=None)
    for project in ['csv_ids', 'url_keys', 'cache_scope']:
        add('run-'+project, 'Executed regression: '+project, 'Methods across projects', 'Python', 'Experiment runner', 'Identity boundaries',
            'Code for a bounded invariant test. Use the separate execution record to inspect exit status; code alone is not proof of execution. The neighboring failed.py preserves the failing implementation.',
            file=(METHOD, 'coding-demo/'+project+'/successful.py'), relative=REL_METHOD, origin='executed constructed code')

    add('anchor-results', 'Source anchors after edits', 'Provenance', 'JSON experiment', 'Experiment runner', 'Evidence recovery',
        '33 controlled mutations of three acquired public documents compare offsets, first exact quote, and unique quote with context. Three originals remain available during these mutations; recovery and abstention are reported separately. Current support still requires revalidation after changes.',
        file=(HERE/'provenance-lab', 'results-initial.json'), relative='../provenance-lab/', origin='measured experiment')
    add('annotation-source', 'Selectors and source state', 'Provenance', 'Public specification', 'W3C', 'Evidence recovery',
        'Primary specification provides selectors and source states; pinned Hypothesis code demonstrates multiple selectors and quote validation. The experiment is an independent simplified resolver.',
        file=(HERE/'provenance-lab', 'source-manifest.json'), relative='../provenance-lab/')

    edges = []
    def edge(a, b, relation, status='recorded'):
        edges.append(dict(source=a, target=b, relation=relation, status=status))
    for a,b,rel in [('motion-thread','motion-after','adopts change'), ('motion-before','motion-after','revised into'),
                    ('motion-after','motion-metrics','produces'),('motion-after','motion-image','produces'),
                    ('motion-after','motion-video','produces'),('motion-source','motion-video','source of'),
                    ('motion-source','motion-image','source of'),('motion-video','motion-image','time anchor'),
                    ('book-page','book-ocr','transcribed into'),('selection-article','selection-code','documents'),
                    ('annotation-source','anchor-results','motivates experiment')]:
        edge(a,b,rel)
    for target in ['book-page','selection-article','selection-code','selection-note']:
        edge('selection-analogy',target,'candidate connection','speculative')
    for thread in demo['threads']:
        edge('method-proposal',thread['id'],'exception' if thread['project']=='signed_urls' else 'candidate shared method','tentative')
        if thread['project']!='signed_urls':
            edge(thread['id'],'run-'+thread['project'],'executed outcome')
    collections = [
        dict(id='motion',title='What changed, why, and what does it show?', nodes=['motion-thread','motion-before','motion-after','motion-metrics','motion-image','motion-video','motion-source'],
             prose='The controlled conversation adopts a bounded detector change. The changed code, measurements, screenshot and video make the change inspectable. They show pixel changes; the material does not establish motives, reliable tracking or a measured accuracy improvement.'),
        dict(id='selection',title='Could keeping alternatives help a future workload?',nodes=['book-page','book-ocr','selection-article','selection-code','selection-note','selection-analogy','selection-failed-link'],
             prose='A real book scan and a public implementation offer a candidate connection to an authored personal note. The unrecorded link is deliberately speculative. The rendered article failed to download; its pinned RST source was captured.'),
        dict(id='method',title='Which method recurs, and where might it fail?',nodes=[x['id'] for x in nodes if x['project']=='Methods across projects'],
             prose='Three constructed coding histories include actual failed and successful programs. They nominate preserving identity and scope before normalization. A fourth thread leaves signed-URL sorting unresolved. Nomination is not automatic promotion into a rule.'),
        dict(id='provenance',title='Can a reader recover the original evidence?',nodes=['annotation-source','anchor-results'],
             prose='Character offsets drift when text changes. Exact quotes help but can land on duplicates. Preserved original revisions permit historical recovery; current claim support remains a separate question.')
    ]
    tasks = [
        dict(id='why-change', required=['motion-thread','motion-after','motion-before'], project='Motion experiment'),
        dict(id='inspect-result', required=['motion-image','motion-video','motion-metrics'], project='Motion experiment'),
        dict(id='check-original', required=['motion-source','motion-image'], project='Motion experiment'),
        dict(id='test-new-connection', required=['selection-analogy','selection-note','book-page','selection-code'], project='Selection and diversity'),
        dict(id='locate-ocr-evidence', required=['book-page','book-ocr'], project='Selection and diversity'),
        dict(id='separate-retrieval-failure', required=['selection-failed-link','selection-article'], project='Selection and diversity'),
        dict(id='method-and-exception', required=['method-proposal','thread-unresolved','run-url_keys'], project='Methods across projects'),
        dict(id='anchor-limitation', required=['anchor-results','annotation-source'], project='Provenance')
    ]
    ids={n['id'] for n in nodes}
    for e in edges:
        assert e['source'] in ids and e['target'] in ids
    for c in collections:
        assert all(i in ids for i in c['nodes'])
    for t in tasks:
        assert all(i in ids for i in t['required'])
    policies = {
        'all_records': lambda n: True,
        'recorded_only': lambda n: n['status']=='recorded',
        'recorded_and_tentative': lambda n: n['status']!='speculative',
        'question_collection': lambda n: True,
    }
    evaluations=[]
    for name,predicate in policies.items():
        for task in tasks:
            visible={n['id'] for n in nodes if predicate(n) and (name!='question_collection' or n['project']==task['project'])}
            recovered=set(task['required'])&visible
            evaluations.append(dict(policy=name,task=task['id'],required=len(task['required']),visible=len(visible),
                                    recovered=len(recovered),complete=len(recovered)==len(task['required']),
                                    extra_visible=len(visible-set(task['required'])),missing=sorted(set(task['required'])-visible)))
    method_messages=sum(len(t['messages']) for t in demo['threads'])
    motion_messages=len(json.loads((MEDIA/'controlled-coding-thread.json').read_text()))
    frame_measurements=len(json.loads((MEDIA/'controlled-demo-metrics.json').read_text()))
    summary=dict(source_cards=len(nodes),typed_edges=len(edges),tasks=len(tasks),
                 promoted_item_baseline=len(nodes)-6+method_messages+motion_messages+frame_measurements,
                 promotion_assumption='Replace five thread cards and one metrics card with their 17 messages and 50 frame records; all other cards unchanged.',
                 api_calls=0,api_cost_usd=0,human_usability_test=False,
                 evaluation_scope='Authored required-node set coverage, not interface reachability, clicks, time, answer correctness or human task completion. Question collections use oracle project membership from the same authored tasks.',
                 policies={p:dict(complete_required_sets=sum(r['complete'] for r in evaluations if r['policy']==p),
                                  mean_visible=sum(r['visible'] for r in evaluations if r['policy']==p)/len(tasks),
                                  mean_extra_visible=sum(r['extra_visible'] for r in evaluations if r['policy']==p)/len(tasks)) for p in policies})
    return dict(research_date=DATE,scope='Public and explicitly constructed research evidence; no private history text or URL records.',nodes=nodes,edges=edges,collections=collections,tasks=tasks),dict(summary=summary,rows=evaluations)


if __name__=='__main__':
    OUT.mkdir(exist_ok=True)
    data, evaluation=build()
    (OUT/'corpus.json').write_text(json.dumps(data,indent=2)+'\n')
    (OUT/'evaluation.json').write_text(json.dumps(evaluation,indent=2)+'\n')
    template=(OUT/'template.html').read_text()
    safe=json.dumps(data).replace('<','\\u003c')
    (OUT/'index.html').write_text(template.replace('/*CORPUS*/',safe))
    print(json.dumps(evaluation['summary'],indent=2))
