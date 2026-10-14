"""Reconcile saved experiment ledgers; never invokes models or reads private data."""
from decimal import Decimal
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read(relative):
    path = ROOT/relative
    return json.loads(path.read_text()), dict(path=relative,sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    records=[]
    sources=[]
    media,source=read('01-source-interpretation/experiments/media-lab/summary.json');sources.append(source)
    history,source=read('02-agent-histories/experiments/aggregate-results.json');sources.append(source)
    connection,source=read('03-connections-and-methods/experiments/audit.json');sources.append(source)
    wiki,source=read('04-navigation-and-provenance/experiments/wiki-lab/analysis.json');sources.append(source)
    def add(concern,role,calls,input_tokens,output_tokens,cost,models):
        records.append(dict(concern=concern,role=role,successful_requests=calls,input_tokens=input_tokens,
                            output_tokens=output_tokens,reported_router_cost_usd=str(Decimal(str(cost))),models=models))
    for role,ledger in media['ledger'].items():
        add('media',role,ledger['successful'],ledger['input_tokens'],ledger['output_tokens'],ledger['router_reported_cost'],ledger['models'])
    h=history['combined'];add('histories','jev',h['successful'],h['input_tokens'],h['output_tokens'],h['cost_usd'],h['model_ids'])
    for role,l in connection['ledger'].items():
        add('connections',role,l['requests'],l['input_tokens'],l['output_tokens'],l['reported_cost_usd'],l['models'])
    for role,l in wiki['ledger'].items():
        add('wiki',role,l['successful_requests'],l['input_tokens'],l['output_tokens'],l['reported_cost_usd'],l['models'])
    def total(rows):
        return dict(successful_requests=sum(r['successful_requests'] for r in rows),
                    input_tokens=sum(r['input_tokens'] for r in rows),output_tokens=sum(r['output_tokens'] for r in rows),
                    reported_router_cost_usd=str(sum((Decimal(r['reported_router_cost_usd']) for r in rows),Decimal('0'))))
    browser=[]
    for p in sorted((ROOT/'04-navigation-and-provenance/experiments/navigation-lab').glob('browser-check*.txt')):
        lines=p.read_text().splitlines()
        result=next((json.loads(x) for x in reversed(lines) if x.startswith('{"schema":')),None)
        browser.append(dict(log=str(p.relative_to(ROOT)),result=result,usage_and_cost_available=False,
                            status='verified' if result and result.get('verified') else 'control_failure_or_unverified'))
    report=dict(research_date='2026-09-29',records=records,sources=sources,
                measured=total(records),jev=total([r for r in records if r['role']=='jev']),
                supporting_and_generative=total([r for r in records if r['role']!='jev']),
                provider_transport_failures=0,
                separate_upstream_metadata=dict(media_upstream_inference_cost_usd=media['ledger']['supporting_model']['upstream_reported_inference_cost'],
                                                byok_calls=media['ledger']['supporting_model']['byok_calls'],
                                                additive_to_router_total=False),
                browser_checks=browser,
                exclusions=['Coding-agent work and subscriptions','Extraction/local runtime','External BYOK account charges',
                            'Browser chooser usage, not exposed by runner','Previous research collection'],
                limitations='Reported router costs are response metadata, not a complete billing reconciliation. Successful transport includes retained invalid/truncated generations. Deterministic experiments incur no model calls.')
    (ROOT/'usage.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['measured','jev','supporting_and_generative','separate_upstream_metadata']},indent=2))


if __name__=='__main__':
    main()
