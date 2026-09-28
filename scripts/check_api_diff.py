#!/usr/bin/env python3
"""Compare API-change magnitude against the actual Elm diff command.

Uses modified docs only in an isolated cache for two registered elm/core versions.
Elm diff refreshes the public package registry; network access is required.
Without --rust-cli this validates engine classification only. With --rust-cli
it compares complete successful output; --ansi captures stdout in a pseudo-terminal.
Error diagnostics are covered separately.
"""
import argparse, copy, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
from types import SimpleNamespace
from diff_test_process import run_command
crate = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--probe', type=Path, default=crate/'target/debug/examples/api_diff')
p.add_argument('--rust-cli', type=Path)
p.add_argument('--report', type=Path, required=True)
p.add_argument('--layouts-only', action='store_true')
p.add_argument('--ansi', action='store_true')
a=p.parse_args()
if a.ansi and not a.rust_cli: p.error('--ansi requires --rust-cli')
binary = a.rust_cli or a.probe
compiler_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
def module():
    return {'name':'Example','comment':'','unions':[],'aliases':[],'values':[],'binops':[]}
def values(tipe):
    m=module();m['values']=[{'name':'value','comment':'','type':tipe}];return [m]
cases=[]
for label,old,new in [
    ('rename','a -> a','b -> b'),('split','a -> a','a -> b'),('merge','a -> b','c -> c'),
    ('widen-number','number -> number','comparable -> comparable'),
    ('narrow-number','comparable -> comparable','number -> number'),
    ('drop-constraint','appendable -> appendable','a -> a'),
    ('add-constraint','a -> a','appendable -> appendable'),
    ('compappend','compappend -> compappend','comparable -> comparable'),
    ('number-prefix','number1 -> number1','number2 -> number2'),
    ('record-order','{ row | a : Int, b : List x }','{ rest | b : List y, a : Basics.Int }'),
    ('record-extension','{ a : Int }','{ row | a : Int }'),
    ('record-field','{ a : Int }','{ b : Int }'),
    ('legacy-name','Posix','Time.Posix'),('qualified-name','A.Token','B.Token'),
    ('tuple-size','(a, b)','(x, y, z)'),('unit','()','()'),
]:cases.append((label,values(old),values(new)))
old=values('a -> a');new=copy.deepcopy(old);new[0]['comment']='new docs';new[0]['values'][0]['comment']='changed comment'
cases.append(('comments',old,new))
new=copy.deepcopy(old);new[0]['values'].append({'name':'extra','comment':'','type':'Int'})
cases.extend([('value-added',old,new),('value-removed',new,old),('module-added',[],old),('module-removed',old,[])])
for label,args,newargs,oldtype,newtype in [
    ('alias-rename',['a','b'],['x','y'],'a -> b','x -> y'),
    ('alias-position',['a','b'],['y','x'],'a -> b','x -> y'),
    ('alias-arity',['a'],['a','b'],'a','a'),
]:
    old=[module()];old[0]['aliases']=[{'name':'Alias','comment':'','args':args,'type':oldtype}]
    new=copy.deepcopy(old);new[0]['aliases'][0].update(args=newargs,type=newtype);cases.append((label,old,new))
old=[module()];old[0]['unions']=[{'name':'Choice','comment':'','args':['a'],'cases':[['One',['a']],['Two',[]]]}]
new=copy.deepcopy(old);new[0]['unions'][0]['cases'].reverse();cases.append(('constructor-order',old,new))
new=copy.deepcopy(old);new[0]['unions'][0]['cases'].append(['Three',[]]);cases.append(('constructor-added',old,new))
new=copy.deepcopy(old);new[0]['unions'][0]['args']=['b'];new[0]['unions'][0]['cases'][0][1]=['b'];cases.append(('union-rename',old,new))
old=[module()];old[0]['unions']=[{'name':'Opaque','comment':'','args':['a'],'cases':[]}]
new=copy.deepcopy(old);new[0]['unions'][0]['args']=['a','b'];cases.append(('opaque-parameters',old,new))
old=[module()];old[0]['binops']=[{'name':'%%','comment':'','type':'a -> a -> a','associativity':'left','precedence':5}]
for field,value in [('precedence',6),('associativity','right')]:
    new=copy.deepcopy(old);new[0]['binops'][0][field]=value;cases.append(('operator-'+field,old,new))
layouts=[]
for label,tipe in [
    ('long-function', 'VeryLongArgumentTypeName -> AnotherLongArgumentTypeName -> YetAnotherLongResultTypeName'),
    ('long-record', '{ firstLongFieldName : List VeryLongArgumentTypeName, secondLongFieldName : Maybe AnotherLongArgumentTypeName }'),
    ('long-open-record', '{ row | firstLongFieldName : List VeryLongArgumentTypeName, secondLongFieldName : Maybe AnotherLongArgumentTypeName }'),
    ('long-tuple', '( List VeryLongArgumentTypeName, Maybe AnotherLongArgumentTypeName, YetAnotherLongResultTypeName )'),
    ('nested-function', '(VeryLongArgumentTypeName -> AnotherLongArgumentTypeName) -> List YetAnotherLongResultTypeName'),
]:
    old=[module()];new=values(tipe)
    layouts.extend([(label+'-added',old,new),(label+'-changed',values('Int'),new)])
for kind,entry in [
    ('aliases', {'name':'LongAliasName','args':['a'],'type':'{ firstLongFieldName : List a, secondLongFieldName : Maybe AnotherLongArgumentTypeName }','comment':''}),
    ('unions', {'name':'LongUnionName','args':['a'],'cases':[['FirstConstructor',['List VeryLongArgumentTypeName','Maybe AnotherLongArgumentTypeName']],['SecondConstructor',['a']]],'comment':''}),
    ('binops', {'name':'%%','type':'VeryLongArgumentTypeName -> AnotherLongArgumentTypeName -> YetAnotherLongResultTypeName','associativity':'left','precedence':5,'comment':''}),
]:
    old=[module()];new=[module()];new[0][kind]=[entry]
    layouts.append(('long-'+kind,old,new))
for width in [71,72,73]:
    layouts.append(('width-'+str(width), [module()], values('A'*(width-8))))
for constructors in [[], [['Only',[]]], [['First',[]],['Second',[]]]]:
    old=[module()];new=[module()]
    new[0]['unions']=[{'name':'NoParameters','args':[],'cases':constructors,'comment':''}]
    layouts.append(('no-parameters-'+str(len(constructors)),old,new))
cases=layouts if a.layouts_only else cases+layouts
results=[]
with tempfile.TemporaryDirectory(prefix='elm-api-diff-') as tmp:
    root=Path(tmp);cache=root/'home/0.19.1/packages';cache.mkdir(parents=True)
    original=Path(os.environ.get('ELM_HOME',Path.home()/'.elm'))/'0.19.1/packages'
    shutil.copy2(original/'registry.dat',cache/'registry.dat')
    paths=[cache/'elm/core'/v/'docs.json' for v in ['1.0.4','1.0.5']]
    for path in paths:path.parent.mkdir(parents=True)
    env={**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1'}
    for label,old,new in cases:
        for path,data in zip(paths,[old,new]):path.write_text(json.dumps(data))
        reference=SimpleNamespace(**run_command([str(a.elm.resolve()),'diff','elm/core','1.0.4','1.0.5'],root,env,a.ansi,'stdout'))
        reference.returncode=reference.code
        probe=SimpleNamespace(**run_command(([str(a.rust_cli.resolve()),'diff','elm/core','1.0.4','1.0.5'] if a.rust_cli else [str(a.probe.resolve()),*map(str,paths)]),root,env,a.ansi,'stdout'))
        probe.returncode=probe.code
        plain=lambda text: re.sub(r'\x1b\[[0-9;]*m','',text)
        match=re.search(r'\b(PATCH|MINOR|MAJOR) change\.',plain(reference.stdout))
        expected=match.group(1) if match else None
        rust_match=re.search(r'\b(PATCH|MINOR|MAJOR) change\.',plain(probe.stdout))
        actual=(rust_match.group(1) if rust_match else None) if a.rust_cli else probe.stdout.strip()
        passed=reference.returncode==0 and probe.returncode==0 and expected==actual and (not a.rust_cli or reference.stdout==probe.stdout)
        results.append({'case':label,'passed':passed,'expected':expected,'actual':actual,'official_stdout':reference.stdout,'official_stderr':reference.stderr,'rust_stderr':probe.stderr,'rust_stdout':probe.stdout})
        print(label, 'PASS' if passed else 'FAIL',flush=True)
assert hashlib.sha256(binary.read_bytes()).hexdigest() == compiler_sha256, 'Compiler changed during validation'
report={'scope':__doc__,'ansi':a.ansi,'cases':len(results),'passed':all(r['passed'] for r in results),'probe_sha256':compiler_sha256,'results':results}
a.report.write_text(json.dumps(report,indent=2)+'\n')
raise SystemExit(not report['passed'])
