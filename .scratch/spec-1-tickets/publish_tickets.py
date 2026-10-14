import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).parent
REPO = 'nicolas-found42/knowledge-garden'
PARENT = f'https://github.com/{REPO}/issues/1'
TICKETS = json.loads((ROOT/'tickets.json').read_text())
BY_KEY = {t['key']: t for t in TICKETS}
MANIFEST = ROOT/'published.json'

def gh(*args):
    return subprocess.run(['gh', *args], text=True, capture_output=True, check=True).stdout

def render(ticket, published):
    gates = []
    for key in ticket['blockers']:
        ref = published.get(key)
        if ref:
            gates.append(f"- [#{ref['number']} — {BY_KEY[key]['title']}]({ref['url']})")
        else:
            gates.append(f"- Draft slice {key}: {BY_KEY[key]['title']}")
    notes = re.sub(r'\b0[1-9]\b', lambda m: '“'+BY_KEY[m.group()]['title']+'”', ticket['notes'])
    criteria = '\n'.join('- [ ] '+c for c in ticket['criteria'])
    stories = ', '.join(str(n) for n in ticket['stories'])
    gates_text = '\n'.join(gates) if gates else 'None (can start immediately).'
    body = f'''## Parent

[Specification #1: automatic Markdown knowledge garden with page search and relational navigation]({PARENT})

Parent coverage: user stories {stories}; **{ticket['section']}**. The parent’s selected decisions take precedence over earlier research recommendations.

## What to build

{ticket['build']}

## Acceptance criteria

{criteria}

## Blocked by

{gates_text}

These are completion gates. `ready-for-agent` means the ticket is specified; implementation starts when its native GitHub blockers are closed.

## Scope and implementation notes

{notes}

Carry the slice through its durable records, Rust/application operations, relevant reader/navigation UI, and verification. Follow the selected Tauri 2 / Rust / TypeScript foundation. Keep lasting knowledge in readable Markdown and original assets in their formats; indexes remain disposable. Preserve shared source identities, evidence, current-value rules, and authoritative external edits as those capabilities become available.

## Verification scenario

{ticket['scenario']}

Use the parent’s application-level boundary: supply or change a temporary source collection, exercise public import/search/navigation/lifecycle actions, and inspect resulting Markdown, retained originals, query results, and visible state. Use deterministic provider responses for integrity/recovery and separate independently labeled live evaluation where model quality is in scope. Each scenario above is a proposed implementation check, not a claim that it has already passed.

## Evidence and validation limits

This is an implementation ticket derived from the [published specification]({PARENT}), not a report of an existing runtime defect. At ticket preparation on September 30, 2026, the repository has engineering guidance but no production application or production test suite. Historical prototypes establish only the bounded observations summarized in #1; local unpublished artifacts are not required to understand this ticket.

The release target is the owner’s 16 GB Apple Silicon Mac (macOS 26.4.1 observed during specification); record the actual build, OS, services, and input coverage when validating. Concrete libraries, helper capabilities, and format limits must be verified during implementation. Do not weaken the selected behavior or introduce user curation to hide a failed validation.

Screenshots/attachments: not applicable at authoring time because this specifies future behavior, not an observed UI failure. No production reproduction, visual evidence, or completed implementation is claimed.
'''
    if ticket['key'] == '35':
        body += '\n## Implementation ticket index and story coverage\n\n'
        body += 'Use native blockers to select work whose prerequisites are complete. The table maps the parent’s stories to delivery paths; overlapping entries represent shared or integrated outcomes. This release ticket also verifies the complete set of 70 stories and the parent’s observable acceptance criteria.\n\n'
        body += '| Parent user stories | Implementation ticket |\n| --- | --- |\n'
        for item in TICKETS[:-1]:
            ref = published.get(item['key'])
            link = f"[#{ref['number']} — {item['title']}]({ref['url']})" if ref else f"Draft slice {item['key']}: {item['title']}"
            body += f"| {', '.join(map(str,item['stories']))} | {link} |\n"
    return body

def lint(ticket, body, final=False):
    headings = ['## Parent','## What to build','## Acceptance criteria','## Blocked by',
                '## Scope and implementation notes','## Verification scenario','## Evidence and validation limits']
    assert all(h in body for h in headings)
    assert [body.index(h) for h in headings] == sorted(body.index(h) for h in headings)
    assert body.count('- [ ] ') == len(ticket['criteria'])
    assert '- [x]' not in body and '![' not in body and '/Users/' not in body
    assert len(body) < 60000
    if final:
        assert 'Draft slice' not in body
    return body

def write_index(published):
    rows = ['# Specification #1 implementation tickets', '', f'Parent: {PARENT}', '',
            'Each ticket includes a complete behavior, observable criteria, a concrete scenario, scope, evidence limits, and native blocking relationships.', '',
            '| Slice | Ticket | Blocked by |', '| --- | --- | --- |']
    for t in TICKETS:
        p = published.get(t['key'])
        title = f"[#{p['number']} — {t['title']}]({p['url']})" if p else t['title']
        gates = ', '.join(f"[#{published[b]['number']}]({published[b]['url']})" if b in published else b for b in t['blockers']) or 'None'
        rows.append(f"| {t['key']} | {title} | {gates} |")
    rows += ['', '## User-story coverage', '', '| Parent story | Tickets |', '| --- | --- |']
    for story in range(1,71):
        covered = [t for t in TICKETS if story in t['stories']]
        links = ', '.join(f"[#{published[t['key']]['number']}]({published[t['key']]['url']})" if t['key'] in published else t['key'] for t in covered)
        rows.append(f'| {story} | {links} |')
    (ROOT/'index.md').write_text('\n'.join(rows)+'\n')

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--publish', action='store_true')
    args = parser.parse_args()
    published = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {}
    (ROOT/'issues').mkdir(exist_ok=True)
    if args.publish:
        parent_now = json.loads(gh('issue','view','1','--repo',REPO,'--json','body,title,comments,labels,state,updatedAt'))
        parent_before = json.loads((ROOT/'parent-before.json').read_text())
        assert parent_now['body'] == parent_before['body'], 'Parent changed: reconcile before publishing.'
        existing = json.loads(gh('issue','list','--repo',REPO,'--state','all','--limit','200','--json','number,title,url,body'))
    for t in TICKETS:
        body = lint(t, render(t,published), final=args.publish)
        filename = ROOT/'issues'/f"{t['key']}.md"
        filename.write_text(body)
        if not args.publish or t['key'] in published:
            continue
        duplicates = [x for x in existing if x['title'] == t['title']]
        assert not duplicates, f'Existing matching issue needs read-back/reconciliation: {duplicates}'
        command = ['issue','create','--repo',REPO,'--title',t['title'],'--body-file',str(filename),
                   '--label','enhancement','--label','ready-for-agent']
        if t['blockers']:
            command += ['--blocked-by',','.join(str(published[b]['number']) for b in t['blockers'])]
        try:
            output = gh(*command).strip()
        except subprocess.CalledProcessError as error:
            (ROOT/'publication-error.json').write_text(json.dumps({'key':t['key'],'stdout':error.stdout,'stderr':error.stderr}, indent=2))
            raise RuntimeError(f"Create may have partially succeeded for {t['key']}; read back before any retry. {error.stdout} {error.stderr}")
        urls = re.findall(r'https://github\.com/'+re.escape(REPO)+r'/issues/\d+', output)
        assert len(urls) == 1, output
        url = urls[0]
        record = dict(number=int(url.rsplit('/',1)[1]),url=url,title=t['title'],key=t['key'])
        published[t['key']] = record
        MANIFEST.write_text(json.dumps(published,indent=2)+'\n')
        current = json.loads(gh('issue','view',str(record['number']),'--repo',REPO,'--json','body,title,labels,blockedBy,state,url'))
        (ROOT/'issues'/f"{t['key']}-published.json").write_text(json.dumps(current,indent=2)+'\n')
        assert current['body'] == body and current['title'] == t['title'], record
        assert {l['name'] for l in current['labels']} == {'enhancement','ready-for-agent'}
        got_blockers = {n['number'] for n in current['blockedBy']['nodes']}
        expected_blockers = {published[b]['number'] for b in t['blockers']}
        assert got_blockers == expected_blockers, (record,got_blockers,expected_blockers)
        print(f"Published and verified {t['key']}: {url} ({len(expected_blockers)} native blockers)",flush=True)
        write_index(published)
    write_index(published)
    print(f"{'Published' if args.publish else 'Drafted'} {len(TICKETS)} tickets; {sum(len(t['blockers']) for t in TICKETS)} native blocking edges.",flush=True)

if __name__ == '__main__':
    main()
