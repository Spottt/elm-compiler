use planexpo_elm::{
    kernel::Mode,
    names::{Space, SymbolKind, Symbols},
    port_codegen, types,
    unify::{Engine, Term},
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn nested_port_records_round_trip_and_reject_bad_input() {
    let mut symbols = Symbols::default();
    let builtins = types::builtins(&mut symbols);
    let integer = builtins.int;
    let mut engine = Engine::new(builtins);
    for module in ["elm/json:Json.Encode", "elm/json:Json.Decode"] {
        for name in [
            "int", "list", "null", "object", "field", "andThen", "succeed", "oneOf", "map",
        ] {
            symbols.intern(module, name, Space::Value, SymbolKind::Value);
        }
    }
    for name in ["Nothing", "Just", "destruct"] {
        symbols.intern("elm/core:Maybe", name, Space::Value, SymbolKind::Value);
    }
    let maybe = symbols.intern(
        "elm/core:Maybe",
        "Maybe",
        Space::Type,
        SymbolKind::Type {
            arity: 1,
            alias: false,
        },
    );
    let list = symbols
        .lookup("elm/core:List", "List", Space::Type)
        .unwrap();
    let int = engine.term(Term::Named(integer, vec![]));
    let maybe = engine.term(Term::Named(maybe, vec![int]));
    let list = engine.term(Term::Named(list, vec![maybe]));
    let record = engine.term(Term::Record {
        fields: [("return".into(), list)].into(),
        extension: None,
    });
    let encoder =
        port_codegen::converter(&mut engine, &symbols, record, false, Mode::Development).unwrap();
    let decoder =
        port_codegen::converter(&mut engine, &symbols, record, true, Mode::Development).unwrap();
    assert!(!encoder.dependencies.is_empty());
    assert!(!decoder.dependencies.is_empty());
    let runtime = r#"
function A2(f,a,b){return f(a)(b);} function A3(f,a,b,c){return f(a)(b)(c);}
function _List_fromArray(a){return a;} function _Utils_Tuple2(a,b){return {a:a,b:b};}
var $elm$core$Maybe$Nothing={$:'Nothing'};
function $elm$core$Maybe$Just(a){return {$:'Just',a:a};}
function $elm$core$Maybe$destruct(n){return f=>v=>v.$==='Nothing'?n:f(v.a);}
var $elm$json$Json$Encode$null=null;
function $elm$json$Json$Encode$int(x){return x;}
function $elm$json$Json$Encode$list(f){return xs=>xs.map(f);}
function $elm$json$Json$Encode$object(xs){return Object.fromEntries(xs.map(x=>[x.a,x.b]));}
function $elm$json$Json$Decode$int(x){if(!Number.isInteger(x))throw Error('bad int');return x;}
function $elm$json$Json$Decode$list(f){return xs=>{if(!Array.isArray(xs))throw Error('bad list');return xs.map(f);};}
function $elm$json$Json$Decode$null(v){return x=>{if(x!==null)throw Error('bad null');return v;};}
function $elm$json$Json$Decode$map(f){return d=>x=>f(d(x));}
function $elm$json$Json$Decode$oneOf(ds){return x=>{for(const d of ds){try{return d(x);}catch(e){}}throw Error('no match');};}
function $elm$json$Json$Decode$field(k){return d=>x=>{if(!(k in x))throw Error('missing field');return d(x[k]);};}
function $elm$json$Json$Decode$andThen(f){return d=>x=>f(d(x))(x);}
function $elm$json$Json$Decode$succeed(v){return x=>v;}
"#;
    let script = format!(
        "{runtime}\nvar encode={},decode={};var input={{return:[3,null]}};var elm=decode(input);if(elm._return[0].a!==3||elm._return[1].$!=='Nothing')throw Error('bad Elm shape');if(JSON.stringify(encode(elm))!==JSON.stringify(input))throw Error('bad roundtrip');var rejected=false;try{{decode({{return:['wrong']}});}}catch(e){{rejected=true;}}if(!rejected)throw Error('accepted bad input');",
        encoder.javascript, decoder.javascript
    );
    let mut child = Command::new("node")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
