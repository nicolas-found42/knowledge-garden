"""Offline source-anchor mutations; no model calls or conversation URL fetching.

Use --fetch-public once only to download the explicitly allowlisted manifest.
This is an independent simplified resolver, NOT a test of Hypothesis's runtime.
"""
import argparse
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).parent
CACHE = Path.home() / '.local/share/knowledge-garden/research/expanded-2026-09-29/public-provenance'
QUOTES = {
    'typesafe-state': 'Jev accepts text only.',
    'w3c-prov': 'A derivation is a transformation of one entity into another.',
    'iiif-presentation': 'An ordered list of Canvases, and/or further Ranges.',
}


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


class VisibleText(HTMLParser):
    def __init__(self):
        super().__init__()
        self.skip = 0
        self.parts = []

    def handle_starttag(self, tag, attrs):
        if tag in ('script', 'style'):
            self.skip += 1

    def handle_endtag(self, tag):
        if tag in ('script', 'style'):
            self.skip = max(0, self.skip - 1)

    def handle_data(self, data):
        if not self.skip:
            self.parts.append(data)


def plain(data):
    parser = VisibleText()
    parser.feed(data)
    return re.sub(r'\s+', ' ', ' '.join(parser.parts)).strip()


def positions(text, quote):
    return [m.start() for m in re.finditer(re.escape(quote), text)]


def mutations(original, quote, start):
    """Return independent controlled mutations and current-target gold positions.

    -1 means no unambiguous unchanged target in the current representation.
    Expected targets are constructed during mutation, never model-generated.
    """
    end = start + len(quote)
    yield 'unchanged', original, start
    banner = 'New document banner. '
    yield 'prepend', banner + original, start + len(banner)
    yield 'append', original + ' New footer.', start
    note = 'Editorial note. '
    yield 'insert_before_quote', original[:start] + note + original[start:], start + len(note)
    yield 'insert_emoji_before', '🌱 ' + original, start + 2
    duplicate = 'Unrelated quotation: ' + quote + '\n'
    yield 'duplicate_quote_before', duplicate + original, start + len(duplicate)
    yield 'duplicate_whole_document', original + '\n' + original, -1
    yield 'delete_quote', original[:start] + original[end:], -1
    yield 'replace_quote', original[:start] + '[Statement withdrawn.]' + original[end:], -1
    yield 'internal_whitespace', original[:start] + quote.replace(' ', '\n') + original[end:], -1
    reversal = 'This is no longer a current assertion: '
    yield 'context_reversal', original[:start] + reversal + original[start:], start + len(reversal)


def evaluate(source_id, original, quote):
    found = positions(original, quote)
    if len(found) != 1:
        raise ValueError(f'{source_id}: expected one source quote, found {len(found)}')
    start = found[0]
    end = start + len(quote)
    prefix, suffix = original[max(0, start - 32):start], original[end:end + 32]
    anchor = dict(source_id=source_id, source_text_sha256=digest(original), quote=quote,
                  start=start, end=end, prefix_sha256=digest(prefix), suffix_sha256=digest(suffix),
                  units='Python Unicode code points in VisibleText/v1 extraction')
    rows = []
    for name, current, gold in mutations(original, quote, start):
        # Assert the mutation's gold position independently before evaluation.
        if gold >= 0:
            assert current[gold:gold + len(quote)] == quote, (source_id, name, gold)
        matches = positions(current, quote)
        exact_context = [p for p in matches if current[max(0, p - len(prefix)):p] == prefix
                         and current[p + len(quote):p + len(quote) + len(suffix)] == suffix]
        context_result = exact_context[0] if len(exact_context) == 1 else -1
        methods = {'position_only': start, 'first_exact_quote': matches[0] if matches else -1,
                   'unique_quote_and_context': context_result}
        row = dict(source=source_id, mutation=name, original_text_sha256=digest(original),
                   current_text_sha256=digest(current), gold_current_start=gold,
                   changed=digest(original) != digest(current), methods={},
                   immutable_original_recovered=original[start:end] == quote,
                   original_recovery_is_current_support=False if name != 'unchanged' else None)
        for method, actual in methods.items():
            row['methods'][method] = dict(actual_start=actual, correct=actual == gold,
                                         wrong_attachment=actual >= 0 and actual != gold,
                                         abstained=actual == -1,
                                         recoverable_missed=actual == -1 and gold >= 0)
        row['current_version_policy'] = 'revalidate' if row['changed'] else 'same_revision'
        rows.append(row)
    return anchor, rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--fetch-public', action='store_true')
    parser.add_argument('--portable', action='store_true', help='Use frozen short public excerpts instead of full cached source text')
    args = parser.parse_args()
    if args.output.exists():
        raise SystemExit('Refusing to overwrite an experiment record')
    manifest = json.loads((ROOT / 'source-manifest.json').read_text())
    CACHE.mkdir(parents=True, exist_ok=True)
    if args.fetch_public:
        for source in manifest['sources']:
            if source['id'] not in QUOTES:
                continue
            request = urllib.request.Request(source['url'], headers={'User-Agent': 'KnowledgeGardenResearch/1'})
            data = urllib.request.urlopen(request, timeout=40).read()
            if hashlib.sha256(data).hexdigest() != source['sha256']:
                raise SystemExit('Public source bytes changed: preserve original manifest and record a new run')
            (CACHE / source['id']).write_bytes(data)
    rows, anchors = [], []
    portable = {f['id']: f['text'] for f in json.loads((ROOT / 'portable-fixtures.json').read_text())}
    for source_id, quote in QUOTES.items():
        text = portable[source_id] if args.portable else plain((CACHE / source_id).read_text())
        anchor, observations = evaluate(source_id, text, quote)
        anchors.append(anchor)
        rows.extend(observations)
    metrics = {}
    for method in rows[0]['methods']:
        observations = [r['methods'][method] for r in rows]
        metrics[method] = {name: sum(r[name] for r in observations) for name in
                           ('correct', 'wrong_attachment', 'abstained', 'recoverable_missed')}
    result = dict(research_date='2026-09-29', source_documents=len(QUOTES),
                  controlled_mutations_per_source=11, observations=len(rows),
                  independent_narrative_cases=False, portable_excerpts=args.portable, model_calls=0, api_cost_usd=0,
                  metrics=metrics, immutable_original_recovered=sum(r['immutable_original_recovered'] for r in rows),
                  changed_revisions_requiring_revalidation=sum(r['changed'] for r in rows),
                  anchors=anchors, rows=rows)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ('rows', 'anchors')}, indent=2))


if __name__ == '__main__':
    main()
