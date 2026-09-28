#!/usr/bin/env python3
"""Compare optimized operators, enums, exhaustive cases and tail switches with Elm."""
import argparse,hashlib,json,os,subprocess,tempfile,shutil
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm',type=Path,required=True)
p.add_argument('--rust',type=Path)
p.add_argument('--report',type=Path,required=True)
a=p.parse_args();crate=Path(__file__).resolve().parents[1]
rust=(a.rust or crate/'target/release/planexpo-elm').resolve()
binaries={'elm':a.elm.resolve(),'rust':rust}
hashes={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in binaries.items()}
source=r'''port module Main exposing (main)
import Platform
import Set
import Task
type Wrap a = Wrap a
unpack (Wrap x) = x
type Color = Red | Green | Blue
type Box = Empty | Box Color (List Int)
type Inner = I Int | J Int
type Outer = Nest Inner | Other
nestedTree x = case x of
    Nest (I 0) -> 10
    Nest (I _) -> 20
    Nest (J _) -> 30
    Other -> 40
port incoming : (Int -> msg) -> Sub msg
port outgoing : List Int -> Cmd msg
pick n = case n of
    0 -> Red
    1 -> Green
    _ -> Blue
color c = case c of
    Red -> 10
    Green -> 20
    Blue -> 30
nested box = case box of
    Empty -> 0
    Box Red [] -> 1
    Box Red (_ :: _) -> 2
    Box Green _ -> 3
    Box Blue xs -> List.length xs
text s = case s of
    "a" -> 1
    "b" -> 2
    _ -> 3
char c = case c of
    'a' -> 1
    'b' -> 2
    _ -> 3
loop n acc = case n of
    0 -> acc
    1 -> loop 0 (acc + 1)
    _ -> loop (n - 1) (acc + 1)
bool b = if b then 1 else 0
calculate n =
    let
        c = pick (remainderBy 3 n)
        xs = [n, n + 1]
        rec = { value = xs }
        empty = []
        partial = (==) rec
    in
    [ color c, nested (Box c xs), nested (Box Red empty), nested Empty
    , bool (rec == { value = xs }), bool (rec /= { value = [] }), bool (partial rec)
    , bool (xs < [n + 1]), bool (xs <= xs), bool (xs > [n - 1]), bool (xs >= xs)
    , n // 3, n // 0, truncate (toFloat n / 2), bool (not False), bool (xor True False)
    , bool (((xs ++ [n]) ++ ([n+1] ++ xs)) == (xs ++ [n,n+1] ++ xs))
    , List.length (xs ++ empty), List.length (empty ++ xs), loop (abs n) 0
    , text "a", text "b", text "other", char 'a', char 'b', char 'z'
    , color (if n == 0 then Red else Blue)
    , nestedTree (Nest (I n)), nestedTree (Nest (J n)), nestedTree Other
    , Tuple.first ((case c of
        Red -> 1
        Green -> 2
        Blue -> 3
      ), n)
    , String.length (String.fromInt n ++ ((String.fromInt n ++ "middle") ++ String.fromInt n))
    , n |> (+) 3 |> (*) 2, (+) 3 <| n, unpack (Wrap n)
    , Set.size (Set.fromList [n,n,n+1]), bool (Set.fromList [n,n+1] == Set.fromList [n+1,n])
    , String.length ("prefix" ++ String.fromInt n ++ "suffix")
    , String.length (String.fromInt n ++ ("x" ++ String.fromInt n))
    ]
main : Program () () Int
main = Platform.worker
    { init = \_ -> ((), Task.perform identity (Task.succeed 100))
    , update = \n model -> (model, outgoing (calculate n))
    , subscriptions = \_ -> incoming identity
    }
'''
runner="""const app=require(process.argv[1]).Elm.Main.init({flags:null}),values=[];app.ports.outgoing.subscribe(x=>values.push(x));for(let n=-30;n<=30;n++)app.ports.incoming.send(n);setTimeout(()=>console.log(JSON.stringify(values)),20);"""
results=[]
with tempfile.TemporaryDirectory(prefix='elm-prod-opt-parity-') as t:
 root=Path(t);shutil.copy(crate/'tests/programs/worker/elm.json',root/'elm.json');(root/'Main.elm').write_text(source)
 env={**os.environ,'GHCRTS':'-N1 -A16m -c'}
 for mode in ['development','production']:
  outputs=[]
  for name,binary in [('elm',a.elm.resolve()),('rust',rust)]:
   out=root/(name+'.js');cmd=[str(binary),'make','Main.elm','--output='+str(out)]+(['--optimize'] if mode=='production' else [])
   compiled=subprocess.run(cmd,cwd=root,env=env,text=True,capture_output=True,timeout=90)
   assert compiled.returncode==0,(name,mode,compiled.stderr)
   result=subprocess.run(['node','-e',runner,str(out)],capture_output=True,text=True,timeout=30)
   assert result.returncode==0,(name,mode,result.stderr)
   outputs.append(json.loads(result.stdout))
   if name=='rust' and mode=='production':
    code=out.read_text();assert 'unreachable Elm pattern' not in code
    assert 'switch(' in code
  assert outputs[0]==outputs[1],(mode,outputs)
  results.append({'mode':mode,'inputs':61,'outputs':outputs[1],'passed':True})
assert hashes=={name:hashlib.sha256(binary.read_bytes()).hexdigest() for name,binary in binaries.items()}
a.report.write_text(json.dumps({'scope':__doc__,'binary_sha256':hashes,'passed':True,'results':results},indent=2)+'\n')
print('Development and production: 61 inputs each match Elm official')
