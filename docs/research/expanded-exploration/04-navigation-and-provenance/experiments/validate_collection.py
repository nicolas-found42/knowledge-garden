"""Offline artifact checks. Reads safe repository artifacts only; no API calls."""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[2]


def main():
    errors=[]
    python_count=json_count=0
    for path in ROOT.rglob('*.py'):
        if '/sources/' in str(path):
            continue
        try:
            ast.parse(path.read_text());python_count+=1
        except Exception as exc:
            errors.append(dict(file=str(path.relative_to(ROOT)),error=type(exc).__name__))
    for path in ROOT.rglob('*.json'):
        try:
            json.loads(path.read_text());json_count+=1
        except Exception as exc:
            errors.append(dict(file=str(path.relative_to(ROOT)),error=type(exc).__name__))
    reports=[ROOT/'README.md',ROOT/'2026-09-29-synthesis.md']+list(ROOT.glob('*/README.md'))+list(ROOT.glob('*/2026-09-29-*.md'))+list(ROOT.glob('*/experiments/**/README.md'))
    links=0
    for path in reports:
        text=re.sub(r'```.*?```','',path.read_text(),flags=re.S)
        text=re.sub(r'`[^`]*`','',text)
        for target in re.findall(r'\]\(([^\n)]+)\)',text):
            target=target.strip('<>').split('#',1)[0]
            if not target or re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:',target):
                continue
            links+=1
            if not (path.parent/target).resolve().exists():
                errors.append(dict(file=str(path.relative_to(ROOT)),broken_target=target))
    nav=ROOT/'04-navigation-and-provenance/experiments/navigation-lab'
    nodes=json.loads((nav/'corpus.json').read_text())['nodes']
    for node in nodes:
        for key in ['href','image','video']:
            if key not in node:
                continue
            path=(nav/node[key]).resolve()
            if not path.is_file():
                errors.append(dict(id=node['id'],missing=node[key]))
            elif key=='href' and node.get('sha256')!=hashlib.sha256(path.read_bytes()).hexdigest():
                errors.append(dict(id=node['id'],hash_mismatch=node[key]))
    scripts=re.findall(r'<script>(.*?)</script>',(nav/'index.html').read_text(),flags=re.S)
    with tempfile.NamedTemporaryFile(mode='w',suffix='.js') as file:
        file.write('\n'.join(scripts));file.flush()
        proc=subprocess.run(['node','--check',file.name],capture_output=True,text=True)
        if proc.returncode:
            errors.append(dict(javascript_syntax=proc.stderr))
    key=os.environ.get('OPENROUTER_API_KEY','').encode()
    secret_findings=[]
    for path in ROOT.rglob('*'):
        if not path.is_file() or path.stat().st_size>=10000000:
            continue
        data=path.read_bytes()
        configured_key=bool(key and key in data)
        pattern=bool(re.search(rb'(?:sk-or-v1-[0-9a-f]{32,}|ghp_[A-Za-z0-9]{30,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----)',data))
        if configured_key or pattern:
            secret_findings.append(str(path.relative_to(ROOT)))
    report=dict(python_files_parsed=python_count,json_files_parsed=json_count,
                local_markdown_links_checked=links,navigation_nodes_checked=len(nodes),
                javascript_syntax_passed=proc.returncode==0,
                credential_pattern_findings=secret_findings,
                privacy_check_limit='Known configured key plus narrow credential patterns; not a proof that arbitrary sensitive content cannot exist.',
                errors=errors)
    (ROOT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
    assert not errors and not secret_findings


if __name__=='__main__':
    main()
