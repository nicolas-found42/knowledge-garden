"""Separate anchor recovery from abstention; exercise actual portable disk lookup."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def resolve(anchor, cache):
    path = cache / (anchor['sha256'] + '.txt')
    if not path.exists():
        return {'status': 'missing_snapshot'}
    data = path.read_bytes()
    if digest(data) != anchor['sha256']:
        return {'status': 'hash_mismatch'}
    text = data.decode('utf8')
    start, end = anchor['start'], anchor['end']
    if start < 0 or end > len(text) or start >= end or text[start:end] != anchor['quote']:
        return {'status': 'invalid_locator'}
    return {'status': 'recovered', 'text': text[start:end], 'source_id': anchor['source_id']}


def main():
    raw = json.loads((ROOT/'results-initial.json').read_text())
    metrics = {}
    for method in raw['metrics']:
        counts = dict(recoverable_targets=0, correct_recoveries=0, missed_recoverable=0,
                      absent_or_ambiguous=0, proper_abstentions=0, wrong_known_target=0,
                      ambiguous_attachments=0, absent_exact_target_attachments=0)
        for row in raw['rows']:
            observation = row['methods'][method]
            if row['gold_current_start'] >= 0:
                counts['recoverable_targets'] += 1
                counts['correct_recoveries'] += observation['correct']
                counts['missed_recoverable'] += observation['abstained']
                counts['wrong_known_target'] += observation['wrong_attachment']
            else:
                counts['absent_or_ambiguous'] += 1
                counts['proper_abstentions'] += observation['abstained']
                if observation['wrong_attachment']:
                    category = 'ambiguous_attachments' if row['mutation']=='duplicate_whole_document' else 'absent_exact_target_attachments'
                    counts[category] += 1
        metrics[method] = counts
    snapshots = ROOT/'snapshots'
    snapshots.mkdir(exist_ok=True)
    results=[]
    # Unlike the initial in-memory slice invariant, this passes through disk and a
    # source registry. Fault injection uses a separate cache and never changes originals.
    faults=ROOT/'fault-injection'
    faults.mkdir(exist_ok=True)
    registry=[]
    for fixture in json.loads((ROOT/'portable-fixtures.json').read_text()):
        text, quote = fixture['text'], fixture['quote']
        content=text.encode()
        hash_=digest(content)
        (snapshots/(hash_+'.txt')).write_bytes(content)
        anchor=dict(source_id=fixture['id'],sha256=hash_,start=text.index(quote),end=text.index(quote)+len(quote),quote=quote)
        registry.append(anchor)
        (faults/(hash_+'.txt')).write_bytes(b'[Deliberately corrupted test copy]')
        cases=[('valid',anchor,snapshots,'recovered'),
               ('missing',{**anchor,'sha256':'0'*64},snapshots,'missing_snapshot'),
               ('corruption',anchor,faults,'hash_mismatch'),
               ('wrong_locator',{**anchor,'start':anchor['start']+1},snapshots,'invalid_locator')]
        for name,ref,cache,expected in cases:
            observed=resolve(ref,cache)
            results.append(dict(source_id=fixture['id'],case=name,expected=expected,actual=observed,correct=observed['status']==expected))
    record=dict(source_registry=registry,metrics=metrics,
                initial_original_invariant='Three original text objects remain in memory; repeated 33 times, not a disk lookup.',
                ambiguity_policy='Duplicate complete document requires abstention; this is not proof that the first occurrence is semantically wrong.',
                whitespace_policy='Only the unchanged exact character sequence is considered recoverable by these simple resolvers.',
                portable_disk_lookup=results,disk_lookup_summary=dict(valid_recovered=sum(r['case']=='valid' and r['correct'] for r in results),
                    invalid_references_rejected=sum(r['case']!='valid' and r['correct'] for r in results)),
                api_calls=0,api_cost_usd=0)
    (ROOT/'audited-results.json').write_text(json.dumps(record,indent=2)+'\n')
    assert all(r['correct'] for r in results)
    print(json.dumps({'metrics':metrics,'disk_lookup':record['disk_lookup_summary']},indent=2))


if __name__=='__main__':
    main()
