"""Test three meanings of time and honest interval ordering; research code."""
from pathlib import Path
import json,datetime,time,platform
R=Path(__file__).resolve().parent;D=json.loads((R/'corpus.json').read_text())
def stamp(s):
    if not s:return None
    d=datetime.datetime.fromisoformat(s.replace('Z','+00:00'))
    return d.replace(tzinfo=datetime.timezone.utc) if not d.tzinfo else d.astimezone(datetime.timezone.utc)
def order(rows,key,descending=False):
    known=[r for r in rows if r.get(key)];unknown=[r for r in rows if not r.get(key)]
    return sorted(known,key=lambda r:(stamp(r[key]),r['id']),reverse=descending)+sorted(unknown,key=lambda r:r['id'])
def before(a,b):
    # Date-only ends are inclusive, so overlapping May interval is never 'before' a May 17 event.
    return bool(a.get('eventEnd') and b.get('eventStart') and stamp(a['eventEnd'])<stamp(b['eventStart']))
def main():
    st=time.perf_counter();items={x['id']:x for x in D['items']};checks=[]
    def check(id,passed,detail):checks.append(dict(id=id,passed=bool(passed),detail=detail))
    check('import-is-not-event',stamp(items['receipt']['activityTime']).year==2026 and stamp(items['receipt']['eventStart']).year==2024,'2024 event, 2026 import remain separate')
    for descending in [False,True]:
        rows=order(D['items'],'eventStart',descending); positions=[i for i,r in enumerate(rows) if not r['eventStart']]
        check('unknown-last-'+str(descending),positions==list(range(len(rows)-len(positions),len(rows))),'Unknown dates stay a separate group regardless of direction')
    check('month-interval-overlap',not before(items['caption'],items['event-visit']) and not before(items['event-visit'],items['caption']),'Cannot order month-only caption relative to May 17')
    check('known-before',before(items['receipt'],items['event-visit']),'May 16 transaction before May 17 visit')
    check('offset-normalization',stamp('2024-05-18T09:00:00Z')==stamp('2024-05-18T05:00:00-04:00'),'UTC instants normalize across zones')
    reason=order(D['reasoning'],'time');check('reasoning-sequence',[x['type'] for x in reason]==['proposal','observation','revision'],'Order is explicitly recorded; not inferred from file modification time')
    check('undated-causal-claim',items['claim-rain']['eventStart'] is None,'A concept or undated claim does not gain event time from source import')
    check('filter-preserves-date', [x['id'] for x in order(D['items'],'eventStart') if x['format']=='PPTX']==['slides'],'Format filtering preserves date semantics')
    out=dict(checks=checks,passed=sum(c['passed'] for c in checks),total=len(checks),seconds=time.perf_counter()-st,platform=platform.platform(),eventAscending=[x['id'] for x in order(D['items'],'eventStart')],activityAscending=[x['id'] for x in order(D['items'],'activityTime')],reasoning=reason,limitation='Earliest-bound display order is not a claim that overlapping intervals are chronologically ordered; UI must expose uncertainty. No natural-language date extraction tested.')
    (R/'timeline-results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
