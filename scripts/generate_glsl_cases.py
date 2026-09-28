#!/usr/bin/env python3
"""Extract the pinned language-glsl 0.3.0 acceptance cases (no Haskell required).

The archive is immutable. Haskell string gaps are removed before JSON decoding
of the simple escape sequences used by this specific test source.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]/'tests/upstream/language-glsl-0.3.0'
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--check', action='store_true')
args = p.parse_args()
for file, sha in json.loads((root/'provenance.json').read_text())['files'].items():
    assert hashlib.sha256((root/file).read_bytes()).hexdigest() == sha, file
source = (root/'tests/Tests.hs').read_text()
source = re.sub(r'\{-.*?-\}', '', source, flags=re.S)
source = re.sub(r'(?m)^--.*$', '', source)
cases = []
for group in ['ExpressionsTrue', 'ExpressionsFalse', 'DeclarationsTrue',
              'DeclarationsFalse', 'FunctionDefinitionsTrue', 'CommentsTrue', 'CommentsFalse']:
    block = re.search('test'+group+r' =\s*\[(.*?)\n  \]', source, re.S).group(1)
    block = re.sub(r'\\\s+\\', '', block)
    for raw in re.findall(r'"(?:[^"\\]|\\.)*"', block):
        text = json.loads(raw)
        if group.startswith('Expressions'):
            shader = 'float value = ('+text+');'
        elif group == 'DeclarationsFalse':
            # Prototypes are rejected by declaration, allowed by externalDeclaration.
            shader = 'void main() { '+text+' }'
        else:
            shader = text
        cases.append({'group':group, 'source':text, 'shader':shader, 'accepted':group.endswith('True')})
cases.append({'group':'Sample', 'source':'glsl/sample-01.glsl',
              'shader':(root/'glsl/sample-01.glsl').read_text(), 'accepted':True})
output = json.dumps(cases, indent=2, ensure_ascii=False)+'\n'
if args.check:
    assert (root/'cases.json').read_text() == output, 'Regenerate cases.json'
else:
    (root/'cases.json').write_text(output)
print(f'{len(cases)} historical GLSL cases verified')
