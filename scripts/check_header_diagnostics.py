#!/usr/bin/env python3
"""Compare implemented header, exposing and declaration report families in JSON and terminals.

Valid layouts compare acceptance; errors compare JSON, plain stderr and ANSI stderr.
"""
import argparse
from compiler_test_support import run_terminal
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--elm', type=Path, required=True)
p.add_argument('--report', type=Path)
p.add_argument('--case-prefix', action='append', default=[], help='Run only case names beginning with a prefix; repeat to combine prefixes')
args = p.parse_args()
crate = Path(__file__).resolve().parents[1]
rust = crate/'target/release/planexpo-elm'
original = Path(os.environ.get('ELM_HOME', str(Path.home()/'.elm')))/'0.19.1/packages'
cases = [
    ('lowercase', 'module main exposing (..)\nvalue = 1\n'),
    ('missing', 'module exposing (..)\nvalue = 1\n'),
    ('number', 'module 123 exposing (..)\nvalue = 1\n'),
    ('underscore', 'module _ exposing (..)\nvalue = 1\n'),
    ('tuple', 'module () exposing (..)\nvalue = 1\n'),
    ('eof', 'module'),
    ('eof-spaces', 'module   '),
    ('eof-newline', 'module\n'),
    ('lower-segment', 'module Alpha.beta exposing (..)\nvalue = 1\n'),
    ('dot-eof', 'module Alpha.'),
    ('dot-space', 'module Alpha. beta exposing (..)\nvalue = 1\n'),
    ('space-before-dot', 'module Alpha .Beta exposing (..)\nvalue = 1\n'),
    ('dot-newline', 'module Alpha.\n Beta exposing (..)\nvalue = 1\n'),
    ('unicode', 'module État.élève exposing (..)\nvalue = 1\n'),
    ('comment-before-name', 'module {- café -} main exposing (..)\nvalue = 1\n'),
    ('indented-name', 'module\n    main exposing (..)\nvalue = 1\n'),
    ('unindented-name', 'module\nmain exposing (..)\nvalue = 1\n'),
    ('line-number-width', '\n'*12 + 'module main exposing (..)\nvalue = 1\n'),
    ('port-lowercase', 'port module main exposing (..)\nvalue = 1\n'),
    ('port-missing', 'port module exposing (..)\nvalue = 1\n'),
    ('port-dot', 'port module Alpha.beta exposing (..)\nvalue = 1\n'),
    ('port-eof', 'port module'),
]
cases.extend([
    ('name-column-one', 'module\nMain exposing (..)\nvalue = 1\n'),
    ('exposing-column-one', 'module Main\nexposing (..)\nvalue = 1\n'),
    ('opening-column-one', 'module Main exposing\n(..)\nvalue = 1\n'),
    ('missing-exposing', 'module Main'),
    ('wrong-exposing', 'module Main expose (..)\nvalue = 1\n'),
    ('comment-before-unindented', 'module Main {- comment -}\nexposing (..)\nvalue = 1\n'),
    ('valid-multiline', 'module\n    Main\n    exposing\n    (..)\nvalue = 1\n'),
    ('root-inside-no-header', 'value = 1'),
    ('root-nested-no-header', 'value = 1'),
    ('root-inside-no-header-missing-import', 'import Missing\nvalue = 1'),
    ('root-outside-no-header', 'value = 1'),
    ('valid-inline', 'module Main exposing (..)\nvalue = 1\n'),
    ('port-keyword-column-one', 'port\nmodule Main exposing (..)\nport send : String -> Cmd msg\n'),
    ('port-name-column-one', 'port module\nMain exposing (..)\nport send : String -> Cmd msg\n'),
    ('port-exposing-column-one', 'port module Main\nexposing (..)\nport send : String -> Cmd msg\n'),
    ('port-opening-column-one', 'port module Main exposing\n(..)\nport send : String -> Cmd msg\n'),
    ('implicit-port-declaration', 'port out : () -> Cmd msg'),
    ('implicit-port-after-value', 'value = 1\nport out : () -> Cmd msg'),
    ('port-prefix-only', 'port'),
    ('port-wrong-keyword', 'port wrong Main exposing (..)\n'),
    ('port-missing-exposing', 'port module Main'),
    ('valid-port-multiline', 'port\n    module\n    Main\n    exposing\n    (..)\nport send : String -> Cmd msg\n'),
])
cases.extend([
    ('cr-before-name', 'module \rmain exposing (..)\nvalue = 1\n'),
    ('cr-before-wrong-exposing', 'module Main \rexpose (..)\nvalue = 1\n'),
    ('cr-name-column-one', 'module\n\rMain exposing (..)\nvalue = 1\n'),
    ('cr-port-column-one', 'port\n\rmodule Main exposing (..)\nport send : String -> Cmd msg\n'),
    ('cr-eof', 'module\r'),
    ('cr-eof-after-newline', 'module\n\r'),
    ('cr-eof-after-comment', 'module {- café\r -}\r'),
    ('cr-eof-in-line-comment', 'module -- café\r'),
    ('cr-comment-before-name', 'module {- café\r -} main exposing (..)\nvalue = 1\n'),
    ('cr-valid-layout', 'module Main exposing (..)\n\rvalue =\r 1\n'),
    ('crlf-valid-layout', 'module Main exposing (..)\r\nvalue = 1\r\n'),
])
for label, listing in [
    ('value-indent', '(\nvalue)'),
    ('end-indent', '(value\n)'),
    ('comma-value-indent', '(value,\nother)'),
    ('comma-end-indent', '(value\n, other)'),
    ('all-value-indent', '(\n..)'),
    ('all-end-indent', '(..\n)'),
    ('type-end-indent', '(T\n(..))'),
    ('type-public-end-indent', '(T(..)\n)'),
    ('missing-comma', '(value other)'),
    ('missing-close', '(value'),
    ('multiline-end', '(\n    value\n    other)'),
    ('multiline-end-indent', '(\n    value\n)'),
]:
    cases.append(('exposing-' + label, 'module Main exposing ' + listing + '\nvalue = 1\nother = 2\ntype T = C\n'))
    cases.append(('import-exposing-' + label, 'module Main exposing (..)\nimport Basics exposing ' + listing + '\nvalue = 1\n'))
cases.extend([
    ('exposing-eof', 'module Main exposing (value'),
    ('exposing-eof-indented', 'module Main exposing (value\n    '),
    ('exposing-eof-newline', 'module Main exposing (value\n'),
    ('exposing-line-width', '\n'*12 + 'module Main exposing (\n    value\n)'),
    ('exposing-comment-indent', 'module Main exposing (value {- café -}\n)'),
    ('exposing-port-indent', 'port module Main exposing (\nvalue)\nvalue = 1'),
])
for label, listing in [
    ('named-variant', '(T(C))'),
    ('empty', '(T())'),
    ('single-dot', '(T(.))'),
    ('three-dots', '(T(...))'),
    ('extra-variant', '(T(.., C))'),
    ('space-dots', '(T(. .))'),
    ('dots-indent', '(T(\n..))'),
    ('close-indent', '(T(..\n))'),
    ('multiline-bad-variant', '(\n    T(C)\n    )'),
    ('comment-indent', '(T( {- café -}\n..))'),
    ('cr-indent', '(T(\n\r..))'),
]:
    cases.append(('privacy-' + label, 'module Main exposing ' + listing + '\ntype T = C\n'))
    cases.append(('import-privacy-' + label, 'module Main exposing (..)\nimport Basics exposing ' + listing + '\nvalue = 1\n'))
cases.extend([
    ('privacy-eof', 'module Main exposing (T('),
    ('privacy-dots-eof', 'module Main exposing (T(..'),
    ('privacy-eof-newline', 'module Main exposing (T(\n'),
    ('privacy-port', 'port module Main exposing (T(C))\ntype T = C'),
])
for label, listing in [
    ('space-before', '(( +))'), ('space-after', '((+ ))'),
    ('comment-before', '(({- comment -}+))'), ('comment-after', '((+{- comment -}))'),
    ('newline-before', '((\n    +))'), ('newline-after', '((+\n    ))'),
    ('name', '((value))'), ('empty', '(())'),
    ('number', '((1))'), ('wrong-close', '((+]'),
    ('dot', '((.))'), ('pipe', '((|))'), ('arrow', '((->))'),
    ('equals', '((=))'), ('colon', '((:))'),
]:
    cases.append(('operator-' + label, 'module Main exposing ' + listing + '\nvalue = 1\n'))
    cases.append(('import-operator-' + label, 'module Main exposing (..)\nimport Basics exposing ' + listing + '\nvalue = 1\n'))
cases.extend([
    ('operator-open-eof', 'module Main exposing (('),
    ('operator-close-eof', 'module Main exposing ((+'),
    ('operator-space-eof', 'module Main exposing ((+ '),
    ('operator-newline-eof', 'module Main exposing ((+\n'),
    ('operator-port', 'port module Main exposing ((=))\nvalue = 1'),
])
cases.append(('operator-double-dot', 'module Main exposing ((..))\nvalue = 1\n'))
for label, listing in [
    ('missing-open', 'value'), ('square-list', '[value]'),
    ('empty-list', '()'), ('trailing-comma', '(value,)'),
    ('double-comma', '(value,, other)'), ('number-value', '(123)'),
    ('string-value', '("x")'), ('record-value', '({})'),
    ('underscore-value', '(_)'), ('reserved-if', '(if)'),
    ('reserved-import', '(import)'), ('reserved-exposing', '(exposing)'),
    ('reserved-after-comma', '(value, type)'), ('bare-plus', '(+)'),
    ('bare-equals', '(=)'), ('bare-arrow', '(->)'),
    ('bare-pipeline', '(|>)'), ('bare-colon', '(:)'),
    ('multiline-empty', '(\n    )'), ('multiline-reserved', '(\n    if)'),
    ('comment-reserved', '({- café -} if)'), ('cr-reserved', '(\rif)'),
]:
    cases.append(('value-' + label, 'module Main exposing ' + listing + '\nvalue = 1\n'))
    cases.append(('import-value-' + label, 'module Main exposing (..)\nimport Basics exposing ' + listing + '\nvalue = 1\n'))
cases.extend([
    ('value-open-eof', 'module Main exposing'),
    ('value-item-eof', 'module Main exposing ('),
    ('value-comma-eof', 'module Main exposing (value,'),
    ('value-port-reserved', 'port module Main exposing (if)\nvalue = 1'),
])
for label, body in [
    ('lower-name', 'import basics\nvalue = 1'),
    ('missing-name', 'import exposing (..)\nvalue = 1'),
    ('number-name', 'import 12\nvalue = 1'),
    ('dot-name', 'import Html.attributes\nvalue = 1'),
    ('dot-space', 'import Html. Attributes\nvalue = 1'),
    ('space-dot', 'import Html .Attributes\nvalue = 1'),
    ('dot-eof', 'import Html.'),
    ('keyword-eof', 'import'),
    ('keyword-newline', 'import\n'),
    ('name-column-one', 'import\nBasics\nvalue = 1'),
    ('lower-alias', 'import Basics as b\nvalue = 1'),
    ('missing-alias', 'import Basics as exposing (..)\nvalue = 1'),
    ('number-alias', 'import Basics as 12\nvalue = 1'),
    ('alias-eof', 'import Basics as'),
    ('alias-column-one', 'import Basics as\nB\nvalue = 1'),
    ('qualified-alias', 'import Basics as Foo.Bar\nvalue = 1'),
    ('wrong-keyword', 'import Basics alias B\nvalue = 1'),
    ('wrong-after-alias', 'import Basics as B export (..)\nvalue = 1'),
    ('list-column-one', 'import Basics exposing\n(..)\nvalue = 1'),
    ('list-after-alias-column-one', 'import Basics as B exposing\n(..)\nvalue = 1'),
    ('name-eof', 'import Basics'),
    ('named-alias-eof', 'import Basics as B'),
    ('list-eof', 'import Basics exposing (..)'),
    ('trailing-comment', 'import Basics -- comment'),
    ('trailing-spaces', 'import Basics\n  '),
    ('comment-indent', 'import {- café -}\nBasics\nvalue = 1'),
    ('cr-name', 'import \rbasics\nvalue = 1'),
    ('multiline-name', 'import\n    basics\nvalue = 1'),
]:
    cases.append(('imports-' + label, 'module Main exposing (..)\n' + body))
for label, body in [
    ('empty', ''), ('comment-only', '{- comment -}\n'),
    ('imports-only', 'import Basics\n'), ('number', '123\n'),
    ('plus', '+\n'), ('arrow', '->\n'), ('underscore', '_\n'),
    ('stray-paren', ')\n'), ('stray-square', ']\n'), ('stray-curly', '}\n'),
    ('then', 'then = 1\n'), ('else', 'else = 1\n'), ('let', 'let = 1\n'),
    ('in', 'in = 1\n'), ('of', 'of = 1\n'), ('where', 'where = 1\n'),
    ('module', 'module Other exposing (..)\n'),
    ('as', 'as B\n'), ('exposing', 'exposing (..)\n'),
    ('late-import', 'value = 1\nimport Basics\n'),
    ('late-stray', 'value = 1\n]\n'),
    ('import-as-column-one', 'import Basics\nas B\nvalue = 1\n'),
    ('import-exposing-column-one', 'import Basics\nexposing (..)\nvalue = 1\n'),
]:
    cases.append(('declaration-' + label, 'module Main exposing (..)\n' + body))
cases.append(('declaration-line-width', 'module Main exposing (..)\n' + '\n'*12 + '}\n'))
for label, body in [
    ('if-expression', 'if True then 1 else 2\n'), ('if-name', 'if = 1\n'),
    ('case-expression', 'case 1 of\n    _ -> 2\n'), ('case-name', 'case = 1\n'),
    ('if-after-import', 'import Basics\nif True then 1 else 2\n'),
    ('case-after-value', 'value = 1\ncase value of\n    _ -> 2\n'),
    ('tuple', '(1, 2)\n'), ('record', '{ field = 1 }\n'),
    ('list', '[1, 2]\n'), ('string', '"hello"\n'),
    ('char', "'x'\n"), ('multiline-string', '"""hello\nworld"""\n'),
    ('list-after-value', 'value = 1\n[1]\n'),
    ('if-line-width', '\n'*12 + 'if True then 1 else 2\n'),
    ('case-after-comment', '{- café -}\ncase 1 of\n    _ -> 2\n'),
]:
    cases.append(('special-declaration-' + label, 'module Main exposing (..)\n' + body))
for index, name in enumerate(['Value', 'Élève', 'İstanbul', 'Σigma', 'ǅelta', 'Kelvin', 'ẞeta', 'Жук', '𐐀name', 'Name_123', 'NAME', 'Name' + 'a'*80, 'Name' + 'a'*200]):
    cases.append(('capital-declaration-' + str(index), 'module Main exposing (..)\n' + name + ' = 1\n'))
cases.extend([
    ('capital-after-import', 'module Main exposing (..)\nimport Basics\nValue = 1\n'),
    ('capital-after-value', 'module Main exposing (..)\nvalue = 1\nOther = 2\n'),
    ('capital-after-comment', 'module Main exposing (..)\n{- café -}\nÉlève = 1\n'),
])
cases.extend([
    ('space-tab-start', '\tmodule Main exposing (..)\nvalue = 1'),
    ('space-tab-module', 'module\tMain exposing (..)\nvalue = 1'),
    ('space-tab-name', 'module Main\texposing (..)\nvalue = 1'),
    ('space-tab-exposing', 'module Main exposing (\t..)\nvalue = 1'),
    ('space-tab-import', 'module Main exposing (..)\nimport\tBasics\nvalue = 1'),
    ('space-tab-value', 'module Main exposing (..)\nvalue =\t1'),
    ('space-tab-comment', 'module Main exposing (..)\n{- café\t -}\nvalue = 1'),
    ('space-tab-nested-comment', 'module Main exposing (..)\n{- outer {- inner\t -} -}\nvalue = 1'),
    ('space-tab-doc-comment', 'module Main exposing (..)\n{-| Overview\t -}\nvalue = 1'),
    ('space-tab-cr', 'module Main exposing (..)\n\r\tvalue = 1'),
    ('space-unclosed-comment-start', '{- unclosed'),
    ('space-unclosed-comment-header', 'module Main {- unclosed'),
    ('space-unclosed-comment-body', 'module Main exposing (..)\nvalue = 1 {- unclosed'),
    ('space-unclosed-comment-nested', 'module Main exposing (..)\n{- outer {- inner -}'),
    ('space-unclosed-comment-multiline', 'module Main exposing (..)\n{- café\nmore'),
    ('space-unclosed-doc', 'module Main exposing (..)\n{-| Overview'),
    ('space-unclosed-comment-cr', 'module Main exposing (..)\n\r{- café'),
    ('space-bare-opener', 'module Main exposing (..)\n{-'),
    ('space-tab-line-comment-valid', 'module Main exposing (..)\n-- comment\t\nvalue = 1'),
])
cases.extend([
    ('priority-name-before-tab', 'module lower exposing (..)\nvalue =\t1'),
    ('priority-exposing-before-comment', 'module Main exposing (value other)\nvalue = 1 {- unfinished'),
    ('priority-indent-before-tab', 'module\nMain exposing (..)\nvalue =\t1'),
    ('priority-alias-before-comment', 'module Main exposing (..)\nimport Basics as lower\nvalue = 1 {- unfinished'),
    ('priority-space-before-value', 'module Main exposing (\n\tvalue)\nvalue = 1'),
    ('priority-tab-after-name', 'module Main\t'),
    ('priority-comment-after-name', 'module Main {- unfinished'),
    ('priority-tab-after-exposing', 'module Main exposing\t'),
    ('priority-tab-after-open', 'module Main exposing (\t'),
    ('priority-comment-after-open', 'module Main exposing ( {- unfinished'),
    ('priority-operator-before-tab', 'module Main exposing ((\t+))\nvalue = 1'),
    ('priority-operator-close-before-tab', 'module Main exposing ((+\t))\nvalue = 1'),
    ('priority-operator-before-comment', 'module Main exposing (({- unfinished'),
    ('priority-operator-close-before-comment', 'module Main exposing ((+{- unfinished'),
    ('priority-private-type-tab', 'module Main exposing (T(\t..))\ntype T = C'),
    ('priority-reserved-before-tab', 'module Main exposing (if)\nvalue =\t1'),
    ('priority-import-list-before-comment', 'module Main exposing (..)\nimport Basics exposing (if)\nvalue = 1 {- unfinished'),
])
for label, suffix in [('third-dot', '.'), ('many-dots', '....'), ('plus', '+'), ('arrow', '->')]:
    cases.append(('wildcard-prefix-module-' + label, 'module Main exposing (..' + suffix + ')\nvalue = 1\n'))
    cases.append(('wildcard-prefix-import-' + label, 'module Main exposing (..)\nimport Basics exposing (..' + suffix + ')\nvalue = 1\n'))
cases.append(('wildcard-prefix-before-tab', 'module Main exposing (...)\nvalue =\t1'))
cases.append(('wildcard-prefix-before-comment', 'module Main exposing (..+)\nvalue = 1 {- unfinished'))

for label, body in [
    ('only', 'infix left 4 (<+>) = combine\n'),
    ('only-no-newline', 'infix left 4 (<+>) = combine'),
    ('two-only', 'infix left 4 (<+>) = combine\ninfix right 5 (<*>) = multiply\n'),
    ('valid', 'infix left 4 (<+>) = combine\ncombine a b = a\n'),
    ('after-value', 'combine a b = a\ninfix left 4 (<+>) = combine\n'),
    ('after-type', 'type Box = Box\ninfix left 4 (<+>) = combine\ncombine a b = a\n'),
]:
    cases.append(('kernel-infix-' + label, 'module Main exposing (..)\n' + body))

for label, declaration in [
    ('associativity', 'infix sideways 4 (<+>) = combine'),
    ('precedence-two-digits', 'infix left 10 (<+>) = combine'),
    ('precedence-negative', 'infix left -1 (<+>) = combine'),
    ('precedence-float', 'infix left 4.5 (<+>) = combine'),
    ('missing-open', 'infix left 4 <+> = combine'),
    ('space-after-open', 'infix left 4 ( <+>) = combine'),
    ('space-before-close', 'infix left 4 (<+> ) = combine'),
    ('comment-after-open', 'infix left 4 ({-x-}<+>) = combine'),
    ('reserved-operator', 'infix left 4 (=) = combine'),
    ('missing-equals', 'infix left 4 (<+>) combine'),
    ('upper-function', 'infix left 4 (<+>) = Combine'),
    ('reserved-function', 'infix left 4 (<+>) = if'),
    ('unindented-assoc', 'infix\nleft 4 (<+>) = combine'),
    ('unindented-precedence', 'infix left\n4 (<+>) = combine'),
    ('unindented-open', 'infix left 4\n(<+>) = combine'),
    ('unindented-equals', 'infix left 4 (<+>)\n= combine'),
    ('unindented-function', 'infix left 4 (<+>) =\ncombine'),
    ('same-line-body', 'infix left 4 (<+>) = combine value = 1'),
    ('valid-multiline', 'infix\n    left\n    4\n    (<+>)\n    =\n    combine'),
]:
    cases.append(('kernel-infix-detail-' + label, 'module Main exposing (..)\n' + declaration + '\ncombine a b = a\n'))

for label, body in [
    ('function-operator', 'f (+) = 1'),
    ('function-comma', 'f (,) = 1'),
    ('function-bracket', 'f (]) = 1'),
    ('function-keyword', 'f (if) = 1'),
    ('function-keyword-case', 'f (case) = 1'),
    ('function-multiline', 'f (\n    +) = 1'),
    ('function-multiline-keyword', 'f (\n    if) = 1'),
    ('function-comment', 'f ({- comment -}+) = 1'),
    ('lambda', 'f = \\(+) -> 1'),
    ('let', 'f = let\n        (+) = 1\n    in 1'),
    ('case', 'f x = case x of\n    (+) -> 1'),
    ('nested', 'f ((+)) = 1'),
    ('eof', 'f ('),
    ('valid-unit', 'f () = 1'),
    ('valid-tuple', 'f (x,y) = x'),
]:
    cases.append(('pattern-paren-open-' + label, 'module Main exposing (..)\n' + body + ('\n' if label != 'eof' else '')))

for label, body in [
    ('extra-name', 'f (x y) = 1'),
    ('tuple-extra-name', 'f (x,y z) = 1'),
    ('keyword', 'f (x if) = 1'),
    ('keyword-then', 'f (x then) = 1'),
    ('plus', 'f (x + y) = 1'),
    ('long-operator', 'f (x <+++> y) = 1'),
    ('equals', 'f (x = 1'),
    ('square', 'f (x] = 1'),
    ('curly', 'f (x} = 1'),
    ('multiline', 'f (x\n    y) = 1'),
    ('multiline-keyword', 'f (x\n    if) = 1'),
    ('comment', 'f (x {- comment -} + y) = 1'),
    ('lambda', 'f = \\(x y) -> 1'),
    ('let', 'f = let\n        (x y) = 1\n    in 1'),
    ('case', 'f x = case x of\n    (y z) -> 1'),
    ('nested', 'f ((x y)) = 1'),
    ('eof', 'f (x'),
    ('valid-cons', 'f (x :: xs) = x'),
]:
    cases.append(('pattern-paren-end-' + label, 'module Main exposing (..)\n' + body + ('\n' if label != 'eof' else '')))

for label, body in [
    ('first', 'f (\nx) = 1'),
    ('unit', 'f (\n) = 1'),
    ('close', 'f (x\n) = 1'),
    ('comma', 'f (x\n,y) = 1'),
    ('second', 'f (x,\ny) = 1'),
    ('third', 'f (x,y,\nz) = 1'),
    ('tuple-close', 'f (x,y\n) = 1'),
    ('eof-newline', 'f (x\n'),
    ('first-eof-newline', 'f (\n'),
    ('second-eof-newline', 'f (x,\n'),
    ('first-comment', 'f ({-comment-}\nx) = 1'),
    ('close-comment', 'f (x {-comment-}\n) = 1'),
    ('valid-first', 'f (\n    x) = x'),
    ('valid-close', 'f (x\n    ) = x'),
    ('valid-tuple', 'f (x,\n    y\n    ) = x'),
]:
    cases.append(('pattern-paren-indent-' + label, 'module Main exposing (..)\n' + body))

for label, body in [
    ('trailing-comma', 'f (x,) = 1'),
    ('double-comma', 'f (x,,y) = 1'),
    ('keyword', 'f (x,if) = 1'),
    ('minus', 'f (x,-1) = 1'),
    ('plus', 'f (x,+) = 1'),
    ('multiline', 'f (x,\n    ) = 1'),
    ('eof', 'f (x,'),
    ('lambda-keyword', 'f = \\(x,if) -> 1'),
    ('let-keyword', 'f = let\n        (x,if) = (1,2)\n    in 1'),
    ('case-keyword', 'f x = case x of\n    (y,if) -> 1'),
    ('case-minus', 'f x = case x of\n    (y,-1) -> 1'),
    ('nested-let-function', 'f = let\n        g (x,if) = 1\n    in 1'),
]:
    cases.append(('pattern-start-' + label, 'module Main exposing (..)\n' + body))

for label, body in [
    ('open-plus', 'f [+] = 1'), ('open-keyword', 'f [if] = 1'),
    ('open-comma', 'f [,] = 1'), ('missing-close', 'f [x y] = 1'),
    ('wrong-close', 'f [x) = 1'), ('end-keyword', 'f [x if] = 1'),
    ('end-plus', 'f [x + y] = 1'), ('trailing-comma', 'f [x,] = 1'),
    ('next-keyword', 'f [x,if] = 1'), ('open-eof', 'f ['), ('end-eof', 'f [x'),
    ('indent-open', 'f [\nx] = 1'), ('indent-empty', 'f [\n] = 1'),
    ('indent-close', 'f [x\n] = 1'), ('indent-next', 'f [x,\ny] = 1'),
    ('open-comment', 'f [{-comment-}\nx] = 1'),
    ('multiline-end', 'f [x\n    y] = 1'),
    ('multiline-open', 'f [\n    +] = 1'),
    ('case-keyword', 'f xs = case xs of\n    [if] -> 1'),
    ('case-next-keyword', 'f xs = case xs of\n    [x,if] -> 1'),
    ('valid', 'f xs = case xs of\n    [x,y] -> x\n    _ -> 0'),
    ('invalid-aligned-case', 'f xs = case xs of\n    [ x\n    , y\n    ] -> x\n    _ -> 0'),
    ('valid-multiline', 'f xs = case xs of\n    [ x\n        , y\n        ] -> x\n    _ -> 0'),
]:
    cases.append(('pattern-list-' + label, 'module Main exposing (..)\n' + body))

for label, body in [
    ('empty', 'f {} = 1'), ('valid', 'f {x,y} = x'),
    ('upper', 'f {X} = 1'), ('open-keyword', 'f {if} = 1'),
    ('open-plus', 'f {+} = 1'), ('open-comma', 'f {,} = 1'),
    ('field-keyword', 'f {x,if} = 1'), ('field-upper', 'f {x,Y} = 1'),
    ('trailing-comma', 'f {x,} = 1'), ('missing-comma', 'f {x y} = 1'),
    ('wrong-close', 'f {x] = 1'), ('end-keyword', 'f {x if} = 1'),
    ('open-eof', 'f {'), ('end-eof', 'f {x'),
    ('indent-open', 'f {\nx} = 1'), ('indent-empty', 'f {\n} = 1'),
    ('indent-close', 'f {x\n} = 1'), ('indent-next', 'f {x,\ny} = 1'),
    ('valid-multiline', 'f {x,\n    y\n    } = x'),
    ('multiline-field-keyword', 'f {x,\n    if} = 1'),
    ('let-keyword', 'f = let\n        {x,if} = {x=1}\n    in 1'),
]:
    cases.append(('pattern-record-' + label, 'module Main exposing (..)\n' + body))

for label, token in [('name','_value'),('upper','_Value'),('double','__value'),('underscores','___'),('digit','_123'),('unicode','_État'),('simple-case','_İtem')]:
    cases.append(('pattern-wildcard-' + label, 'module Main exposing (..)\nf ' + token + ' = 1'))
for label, body in [
    ('tuple','f (x,_value) = 1'), ('list','f [_value] = 1'),
    ('lambda','f = \\_value -> 1'),
    ('case','f x = case x of\n    _value -> 1'),
    ('let','f = let\n        (_value,x) = (1,2)\n    in 1'),
    ('valid','f _ = 1'),
]:
    cases.append(('pattern-wildcard-' + label, 'module Main exposing (..)\n' + body))

for label, body in [
    ('upper','f (x as Name) = 1'), ('keyword','f (x as if) = 1'),
    ('wildcard','f (x as _) = 1'), ('named-wildcard','f (x as _name) = 1'),
    ('number','f (x as 1) = 1'), ('close','f (x as) = 1'),
    ('eof','f (x as'), ('indent','f (x as\nname) = 1'),
    ('indent-comment','f (x as {-comment-}\nname) = 1'),
    ('valid','f ((x,y) as pair) = pair'),
    ('valid-multiline','f (x as\n    name) = name'),
    ('case','f x = case x of\n    y as Name -> 1'),
]:
    cases.append(('pattern-alias-' + label, 'module Main exposing (..)\n' + body))

for label, number in [('decimal','1.0'), ('exponent','1e3'), ('fraction-exponent','1.25e-3'), ('uppercase-exponent','1E+3'), ('zero','0.0')]:
    cases.append(('pattern-float-' + label, 'module Main exposing (..)\nf ' + number + ' = 1'))
for label, body in [
    ('tuple','f (x,1.0) = 1'), ('list','f [1.0] = 1'),
    ('case','f x = case x of\n    1.0 -> 1'), ('lambda','f = \\1.0 -> 1'),
    ('let','f = let\n        (1.0,x) = (1,2)\n    in 1'),
    ('integer','f x = case x of\n    1 -> 1\n    _ -> 0'),
    ('hex','f x = case x of\n    0xFE -> 1\n    _ -> 0'),
]:
    cases.append(('pattern-float-' + label, 'module Main exposing (..)\n' + body))

for label, body in [
    ('close','f (x ::) = 1'), ('keyword','f (x :: if) = 1'),
    ('minus','f (x :: -1) = 1'), ('eof','f (x ::'),
    ('newline-eof','f (x ::\n'), ('indent','f (x ::\nxs) = 1'),
    ('comment','f (x :: {-comment-}\nxs) = 1'),
    ('case-keyword','f x = case x of\n    y :: if -> 1'),
    ('case-indent','f x = case x of\n    y ::\n    ys -> 1'),
    ('multiline-indent','f (x\n    ::\nxs) = 1'),
    ('valid','f xs = case xs of\n    x :: rest -> x\n    _ -> 0'),
    ('valid-multiline','f xs = case xs of\n    x ::\n        rest -> x\n    _ -> 0'),
]:
    cases.append(('pattern-cons-' + label, 'module Main exposing (..)\n' + body))

effect_tokens = ['effect', 'module', 'Main', 'where', '{', 'command', '=', 'Cmd', ',', 'subscription', '=', 'Sub', '}', 'exposing', '(..)']
for split in range(1, len(effect_tokens)):
    cases.append((f'effect-layout-{split}', ' '.join(effect_tokens[:split])+'\n'+' '.join(effect_tokens[split:])+'\nvalue = 1\n'))
for label, source in [
    ('eof', 'effect'), ('missing-module','effect Main'),
    ('empty-manager','effect module Main where {} exposing (..)'),
    ('unknown-field','effect module Main where { foo = Cmd } exposing (..)'),
    ('duplicate-command','effect module Main where { command = Cmd, command = Cmd } exposing (..)'),
    ('lower-type','effect module Main where { command = cmd } exposing (..)'),
    ('bad-exposing','effect module Main where { command = Cmd } exposing ()'),
    ('missing-where','effect module Main exposing (..)'),
]:
    cases.append(('effect-'+label,source+'\nvalue = 1\n'))
for label, exposing in [('trailing-comma','(value,)'), ('reserved-operator','((=))'), ('unclosed','(value'), ('variant','(T(C))')]:
    cases.append(('effect-exposing-'+label,'effect module Main where { command = Cmd } exposing '+exposing+'\nvalue = 1\n'))
for prefix in ['application-effects-', 'package-effects-']:
    for label, source in [
        ('unexpected-port','module Main exposing (..)\nport send : Int -> Cmd msg\n'),
        ('no-ports','port module Main exposing (..)\nvalue = 1\n'),
        ('port-module','port module Main exposing (..)\nport send : Int -> Cmd msg\n'),
        ('manager','effect module Main where { command = Cmd } exposing (..)\nvalue = 1\n'),
        ('multiline','port\n    module Main exposing (..)\nvalue = 1\n'),
    ]:
        cases.append((prefix+label,source))
for index, literal in enumerate(['01','000','0x','0xG','0x12g','1foo','0e2','2.','0.','123.','18446744073709551616.','1e','1e+','1e-','1.2e+','3_']):
    for context in ['expression','pattern']:
        body = 'value = '+literal if context == 'expression' else 'f '+literal+' = 1'
        cases.append((f'number-{context}-{index}','module Main exposing (..)\n'+body+'\n'))
cases.append(('number-exponent-tail','module Main exposing (..)\nvalue = 1e2identity\n'))
for index, literal in enumerate(['"hello', '"', '"""hello\nworld', "'", "'a", "''", "'ab'", "'éø'", '"hello\\', "'a\\", '"bad\\q"', "'\\q'", '"""bad\\q"""']):
    for context in ['expression','pattern']:
        body = 'value = '+literal if context == 'expression' else 'f '+literal
        cases.append((f'literal-{context}-{index}','module Main exposing (..)\n'+body+'\n'))
for index, source in enumerate(['module Main exposing (..)\nvalue = "abc\\', "module Main exposing (..)\nvalue = 'a\\", 'module Main exposing (..)\nvalue = """abc\\', 'module Main exposing (..)\nvalue = "a\rb"\n', "module Main exposing (..)\nvalue = '\r'\n"]):
    cases.append((f'literal-eof-cr-{index}',source))
for index, escape in enumerate([r'\u',r'\u0041',r'\u{',r'\u{}',r'\u{g}',r'\u{12g}',r'\u{1}',r'\u{01}',r'\u{001}',r'\u{0000000}',r'\u{110000}',r'\u{FFFFFF}',r'\u{8000000000000000}',r'\u{10000000000000000}',r'\u{0000}',r'\u{0041}',r'\u{D800}',r'\u{10FFFF}',r'\u{000041}']):
    for context in ['string','char','multi','pattern']:
        quote = "'" if context in ['char','pattern'] else ('"""' if context == 'multi' else '"')
        literal = quote+escape+quote
        body = 'f '+literal+' = 1' if context == 'pattern' else 'value = '+literal
        cases.append((f'unicode-escape-{context}-{index}','module Main exposing (..)\n'+body+'\n'))
for index, literal in enumerate(['01','0x','"unfinished',"'ab'",r'"\u{1}"',r'"\q"']):
    for label, prefix in [('name','module main exposing (..)\nvalue = '),('exposing','module Main exposing (value,)\nvalue = '),('import','module Main exposing (..)\nimport lowercase\nvalue = ')]:
        cases.append((f'priority-literal-{label}-{index}',prefix+literal+'\n'))
    for label, prefix in [('name','module '),('exposing','module Main exposing ('),('import','module Main exposing (..)\nimport ')]:
        cases.append((f'priority-boundary-{label}-{index}',prefix+literal+'\n'))
for index, suffix in enumerate(['01','0x','"unfinished',"'ab'",r'"\u{1}"',r'"\q"','\t','{- unclosed']):
    for label, body in [('capital','Capital = 1\nvalue = '),('wildcard','f _name = '),('float','f 1.5 = '),('reserved','f (if) = '),('list','f [x,] = '),('valid-prefix','value = ')]:
        cases.append((f'priority-body-{label}-{index}','module Main exposing (..)\n'+body+suffix+'\n'))
for index, literal in enumerate(['01','0x','"unfinished',"'ab'",r'"\u{1}"',r'"\q"']):
    for label, prefix, suffix in [('declaration','',''),('record-open','f {','} = 1'),('record-field','f { x,','} = 1'),('alias','f (x as ',') = 1'),('pattern-valid','f ',' = 1'),('expression-valid','value = [',']')]:
        cases.append((f'priority-token-{label}-{index}','module Main exposing (..)\n'+prefix+literal+suffix+'\n'))
for index, literal in enumerate(['0x8000000000000000','0xFFFFFFFFFFFFFFFF','0x18000000000000000','0x10000000000000000']):
    cases.append((f'hex-overflow-expression-{index}','module Main exposing (..)\nvalue = '+literal+'\n'))
    cases.append((f'hex-overflow-pattern-{index}','module Main exposing (..)\nf x = case x of\n    '+literal+' -> 1\n    _ -> 0\n'))
for index, literal in enumerate(['0xFFFFFFFFFFFFFFFF','0xffffffffffffffff','0x0FFFFFFFFFFFFFFFF','0xFFFFFFFFFFFFFFFFF']):
    cases.append((f'hex-sentinel-{index}','module Main exposing (..)\nvalue = '+literal+'\n'))
for index, after in enumerate(['= 1', ': xs -> 1', 'if True then 1 else 2', 'let x = 1 in x', 'else 1', ')', '42', 'next', '+ 1']):
    for context in ['single','multi']:
        body = 'value = case input of _ '+after if context == 'single' else 'value =\n    case input of\n        _ '+after
        cases.append((f'case-arrow-{context}-{index}','module Main exposing (..)\n'+body+'\n'))
for index, tail in enumerate(['', '\n', '\n        -> 1', '\n    -> 1', '\n-> 1', '\n        = 1', ' {- comment -}\n    -> 1']):
    cases.append((f'case-indent-arrow-{index}', 'module Main exposing (..)\nf input =\n    case input of\n        _'+tail+'\n'))
for phase, prefix in [('expr','case'), ('of','case input'), ('pattern','case input of'), ('branch','case input of\n        _ ->')]:
    for index, tail in enumerate(['', '\n', '\nnext = 1', ' {- comment -}\nnext = 1']):
        cases.append((f'case-indent-{phase}-{index}', 'module Main exposing (..)\nf input =\n    '+prefix+tail+'\n'))
for index, token in enumerate(['then', 'else', 'in', '=', '->', ')', ']', '}']):
    for context, separator in [('single',' '),('multi','\n        ')]:
        cases.append((f'case-missing-of-{context}-{index}', 'module Main exposing (..)\nf input =\n    case input'+separator+token+'\n'))
for phase, prefix in [('condition','if'), ('then','if True'), ('then-branch','if True then'), ('else-branch','if True then 1 else')]:
    for index, tail in enumerate(['', '\n', '\nnext = 1', ' {- comment -}\nnext = 1']):
        cases.append((f'if-indent-{phase}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+tail+'\n'))
for phase, prefix in [('then','if True'),('else','if True then 1')]:
    for index, token in enumerate(['in',')',']','}']):
        for context, sep in [('single',' '),('multi','\n        ')]:
            cases.append((f'if-keyword-{phase}-{context}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+sep+token+'\n'))
for index, tail in enumerate(['','\n','\nnext = 1','\nelse 2','\n\nelse 2',' {- comment -}\nelse 2','\nelsewhere = 2','\nelse_ = 2','\nelse١ = 2','\nelse² = 2']):
    cases.append((f'if-else-indent-{index}', 'module Main exposing (..)\nvalue =\n    if True then 1'+tail+'\n'))
for index, token in enumerate(['then','else','in','of',')',']','}',',','->','=','+','_','(', '[', '{', 'case', 'let', '\\']):
    cases.append((f'if-else-start-{index}', 'module Main exposing (..)\nvalue =\n    if True then 1 else '+token+'\n'))
for index, tail in enumerate(['if', 'if True', 'if True then', 'if True then 1', 'if True then 1 else', 'if True then 1 else )', 'if True )', 'if True then 1 )']):
    for context, prefix in [('chain','if False then 0 else\n        '),('double','if False then 0 else\n        if False then 0 else\n            '),('parenthesized','if False then 0 else (\n        ')]:
        cases.append((f'if-chain-{context}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+tail+'\n'))
for phase, prefix in [('open','('),('end','(1'),('next','(1,'),('third','(1,2,')]:
    for index, tail in enumerate(['', '\n', '\nnext = 1', ' {- comment -}\nnext = 1']):
        cases.append((f'tuple-indent-{phase}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+tail+'\n'))
for context, prefix in [('single','(1 '),('multi','(1,\n        2 '),('triple','(1,2,3 ')]:
    for index, token in enumerate(['then','else','in','of',']','}']):
        cases.append((f'tuple-close-{context}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+token+'\n'))
for index, op in enumerate(['+','*','::','|>','&&','++']):
    for context, tail in [('eof',''),('space',' )'),('argument','1)'),('line','\n        )'),('valid',')')]:
        cases.append((f'tuple-operator-{context}-{index}', 'module Main exposing (..)\nvalue = ('+op+tail+'\n'))
for index, op in enumerate(['|','->','=',':']):
    for context, tail in [('eof',''),('closed',')'),('space',' )'),('argument','1)'),('line','\n        )')]:
        cases.append((f'tuple-reserved-{context}-{index}', 'module Main exposing (..)\nvalue = ('+op+tail+'\n'))
for index, tail in enumerate(['', ' ', ' )', '1)', '(1))', '+)', 'then)', '\n        )', '. )', '[)', '{)', 'x)', ')']):
    cases.append((f'tuple-minus-{index}', 'module Main exposing (..)\nvalue x = (-'+tail+'\n'))
for context, prefix in [('function','.'),('variable','record.'),('parentheses','(record).'),('record','{ name = 1 }.')]:
    for index, tail in enumerate(['',' name','Name','then','_name','1','\n        name','name']):
        cases.append((f'accessor-{context}-{index}', 'module Main exposing (..)\nvalue record = '+prefix+tail+'\n'))
for word in ['if','then','else','case','of','let','in','type','module','where','import','exposing','as','port']:
    for prefix in ['Basics','Example.Nested']:
        cases.append((f'qualified-keyword-{prefix}-{word}', 'module Main exposing (..)\nvalue = '+prefix+'.'+word+'\n'))
for phase, prefix in [('open','['),('end','[1'),('next','[1,'),('third','[1,2,')]:
    for index, tail in enumerate(['', '\n', '\nnext = 1', ' {- comment -}\nnext = 1']):
        cases.append((f'list-indent-{phase}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+tail+'\n'))
for phase, prefix in [('open','['),('end','[1 '),('entry','[1,')]:
    for index, tail in enumerate(['then',')','}',']']):
        cases.append((f'list-token-{phase}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail+'\n'))
for phase, prefix in [('open','{'),('equals','{ x'),('expr','{ x ='),('field','{ x = 1,'),('update','{ record |')]:
    for index, tail in enumerate(['', '\n', '\nnext = 1', ' {- comment -}\nnext = 1']):
        cases.append((f'record-indent-{phase}-{index}', 'module Main exposing (..)\nvalue record =\n    '+prefix+tail+'\n'))
for context, prefix in [('literal','{ x = 1'),('update','{ record | x = 1')]:
    for index, tail in enumerate(['', '\n', '\nnext = 1', '\n}', '\n\n}', ' {- comment -}\n}', '\n}extra', '\n    }']):
        cases.append((f'record-end-indent-{context}-{index}', 'module Main exposing (..)\nvalue record =\n    '+prefix+tail+'\n'))
for context, prefix in [('first','{ x '),('second','{ x = 1, y '),('update','{ record | x ')]:
    for index, tail in enumerate(['then','else',': 1','}',',','\n        )']):
        cases.append((f'record-equals-{context}-{index}', 'module Main exposing (..)\nvalue record =\n    '+prefix+tail+'\n'))
for context, prefix in [('literal','{ x = 1 '),('multi','{ x = 1,\n        y = 2 '),('update','{ record | x = 1 ')]:
    for index, tail in enumerate(['then','else','in','of',')',']']):
        cases.append((f'record-close-{context}-{index}', 'module Main exposing (..)\nvalue record =\n    '+prefix+tail+'\n'))
for context, prefix in [('open','{ '),('field','{ x = 1, '),('update','{ record | ')]:
    for index, tail in enumerate(['if','then','case','Name','_name','42',')',',','}','name = 1 }']):
        cases.append((f'record-field-{context}-{index}', 'module Main exposing (..)\nvalue record =\n    '+prefix+tail+'\n'))
for context, prefix in [('parentheses','(1'),('list','[1'),('record','{ x = 1')]:
    for index, tail in enumerate(['', ' ', '   ', ' {- comment -}', '\n', '\n    ']):
        cases.append((f'closing-eof-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))

for phase, prefix in [('argument', '\\'), ('arrow', '\\x'), ('body', '\\x ->')]:
    for index, tail in enumerate(['\n', '\nnext = 1\n', ' {- comment -}\n', '\n-- comment\nnext = 1\n']):
        cases.append((f'lambda-indent-{phase}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))


for context, prefix in [('single', '\\x '), ('multiple', '\\x y '), ('nested', '(\\x ')]:
    for index, tail in enumerate(['if', 'then', 'else', '=', ':', ')', '']):
        cases.append((f'lambda-arrow-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))


for context, prefix in [('plain', '\\'), ('nested', '(\\')]:
    for index, tail in enumerate(['if', 'then', '-> 1', ')', ']', '=', '', '   ']):
        cases.append((f'lambda-first-argument-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))


for phase, prefix in [('definition', 'let'), ('body', 'let x = 1 in')]:
    for index, tail in enumerate(['\n', '\nnext = 1\n', ' {- comment -}\n', '\n-- comment\nnext = 1\n']):
        cases.append((f'let-indent-{phase}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))


for context, prefix in [('inline', 'let x = 1'), ('multiline', 'let\n        x = 1')]:
    for index, tail in enumerate(['\n', '\nnext = 1\n', ' {- comment -}\n', '\n    nope', '\n    y = 2', '\n    )', '']):
        cases.append((f'let-in-{context}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+tail))


for context, prefix in [('inline', 'let '), ('multiline', 'let\n        ')]:
    for index, tail in enumerate(['if', 'then', 'in', 'case', ')', ']', '=', '', '   ']):
        cases.append((f'let-name-{context}-{index}', 'module Main exposing (..)\nvalue =\n    '+prefix+tail))


for phase, prefix in [('name','item'), ('argument','item x'), ('type','item :'), ('body','item =')]:
    for index, tail in enumerate(['\n', '\n    in 1\n', ' {- comment -}\n', '\n    other = 1\n']):
        cases.append((f'let-definition-indent-{phase}-{index}', 'module Main exposing (..)\nvalue =\n    let\n        '+prefix+tail))


for context, prefix in [('value','item '), ('function','item x '), ('multiline','item\n            x ')]:
    for index, tail in enumerate(['if', 'then', 'as', 'in', '->', '+', '::', ')', ']', '', 'exposing', 'module', 'type', 'alias']):
        cases.append((f'let-definition-equals-{context}-{index}', 'module Main exposing (..)\nvalue =\n    let\n        '+prefix+tail))


for shape in ['(x,y)', '{ x }', '[x]']:
    for index, tail in enumerate(['\n', ' =\n', ' : Int', ' -> 1', ' then', '']):
        cases.append((f'let-destruct-{shape}-{index}', 'module Main exposing (..)\nvalue =\n    let\n        '+shape+tail))


for context, tail in [('missing',''), ('in','    in 1'), ('keyword','        if'), ('different','        other = 1'), ('different-broken','        other = ('), ('dedented','    item = 1'), ('valid','        item = 1\n    in item')]:
    cases.append((f'let-annotation-{context}', 'module Main exposing (..)\nvalue =\n    let\n        item : Int\n'+tail))


for context, prefix in [('top','value = '), ('lambda','value = \\x -> '), ('local','value = let item = '), ('destruct','value = let (x,y) = '), ('if-condition','value = if '), ('if-then','value = if True then '), ('case-subject','value = case '), ('case-branch','value = case 1 of _ -> '), ('record','value = { x = ')]:
    for index, tail in enumerate(['then', ')', '+', '', '->']):
        cases.append((f'expression-start-{context}-{index}', 'module Main exposing (..)\n'+prefix+tail))


for context, prefix in [('first','( '), ('second','(1, '), ('third','(1,2, ')]:
    for index, tail in enumerate(['then', '+', '', ')', '->']):
        cases.append((f'paren-expression-start-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))


for context, prefix in [('list-first','[ '), ('list-next','[1, '), ('let-body','let x = 1 in '), ('lambda-let','\\x -> let y = x in '), ('record-let','{ x = let y = 1 in '), ('paren-let','(let x = 1 in ')]:
    for index, tail in enumerate(['then', '->', '']):
        cases.append((f'inherited-expression-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+tail))


for context, prefix, suffix in [('top','', ''), ('list','[', ']'), ('parens','( ', ')')]:
    for index, expression in enumerate(['-.name', '-.', '-.Name', '-. name']):
        cases.append((f'minus-accessor-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+expression+suffix))


for index, expression in enumerate(['-.name', '-.Name', '-.']):
    cases.append((f'minus-accessor-lexical-{index}', 'module Main exposing (..)\nvalue = '+expression+'\nlater = "\\q"\n'))


for context, prefix in [('top',''), ('list','[ '), ('parens','( '), ('operator','(')]:
    for index, tail in enumerate(['', ' 1', ' {- comment -}1', '\n    1']):
        cases.append((f'minus-gap-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+'-'+tail))


for context, prefix in [('top',''), ('list','[ '), ('parens','( '), ('else','if True then 1 else ')]:
    for index, operator in enumerate(['-+', '-*', '-::', '->>', '-=', '-/', '-|', '-!']):
        cases.append((f'minus-symbol-{context}-{index}', 'module Main exposing (..)\nvalue = '+prefix+operator))

if args.case_prefix:
    cases = [case for case in cases if case[0].startswith(tuple(args.case_prefix))]
    if not cases:
        p.error('No diagnostic cases match the requested prefixes')
results, failures, reference_invalid_json = [], [], []
with tempfile.TemporaryDirectory(prefix='elm-header-diagnostics-') as directory:
    root = Path(directory)
    cache = root/'home/0.19.1/packages'
    cache.mkdir(parents=True)
    shutil.copyfile(original/'registry.dat', cache/'registry.dat')
    for package, version in [('elm/core','1.0.5'), ('elm/json','1.1.3')]:
        shutil.copytree(original/package/version/'src', cache/package/version/'src')
        shutil.copyfile(original/package/version/'elm.json', cache/package/version/'elm.json')
    env = {**os.environ,'ELM_HOME':str(root/'home'),'GHCRTS':'-N1 -A16m -c','no_proxy':'','NO_PROXY':''}
    for key in ['http_proxy','https_proxy','all_proxy','HTTP_PROXY','HTTPS_PROXY','ALL_PROXY']:
        env[key] = 'http://127.0.0.1:9'
    for name, source in cases:
        project = root/name
        project.mkdir()
        shutil.copyfile(crate/'tests/programs/worker/elm.json', project/'elm.json')
        if name.startswith(('kernel-infix-', 'package-effects-')):
            config = {'type':'package','name':'elm/syntax-fixture','summary':'Compiler syntax compatibility fixture.',
                      'license':'BSD-3-Clause','version':'1.0.0','exposed-modules':['Main'],
                      'elm-version':'0.19.0 <= v < 0.20.0',
                      'dependencies':{'elm/core':'1.0.0 <= v < 2.0.0'},'test-dependencies':{}}
            if name.startswith('package-effects-'):
                config['name'] = 'author/syntax-fixture'
            (project/'elm.json').write_text(json.dumps(config))
            (project/'src').mkdir()
        entry = 'Nested/Main.elm' if name == 'root-nested-no-header' else 'Main.elm'
        if name.startswith(('kernel-infix-', 'package-effects-')):
            entry = 'src/Main.elm'
        (project/entry).parent.mkdir(parents=True, exist_ok=True)
        (project/entry).write_text(source)
        if name == 'root-outside-no-header':
            config = json.loads((project/'elm.json').read_text())
            config['source-directories'] = ['src']
            (project/'src').mkdir()
            (project/'elm.json').write_text(json.dumps(config))
        command = ['make',entry,'--output=/dev/null']
        runs = [subprocess.run([str(binary),*command,'--report=json'],cwd=project,env=env,capture_output=True,text=True,timeout=30) for binary in [args.elm.resolve(),rust]]
        raw_reference_json = None
        try:
            expected = json.loads(runs[0].stderr) if runs[0].returncode else None
        except json.JSONDecodeError as error:
            if 'Invalid control character' not in str(error):
                raise
            # Elm embeds raw tabs in some source snippets. Preserve the defect
            # as evidence; only the reference decoder permits control bytes.
            expected = json.loads(runs[0].stderr, strict=False)
            raw_reference_json = runs[0].stderr
            reference_invalid_json.append(name)
        actual = json.loads(runs[1].stderr) if runs[1].returncode else None
        titles = [p['title'] for e in (expected or {}).get('errors',[]) for p in e['problems']]
        if name.startswith('minus-accessor-') and '-lexical-' not in name and name.endswith('-0'):
            parsed = subprocess.run([str(rust), 'parse', str(project/entry)], env=env, capture_output=True, text=True, timeout=30)
            actual_titles = [p['title'] for e in (actual or {}).get('errors',[]) for p in e['problems']]
            if parsed.returncode != 0 or titles != ['TYPE MISMATCH'] or actual_titles != ['TYPE MISMATCH']:
                failures.append(name + ': accessor negation reaches type checking')
        if name.startswith('qualified-keyword-'):
            parsed = subprocess.run([str(rust), 'parse', str(project/entry)], env=env, capture_output=True, text=True, timeout=30)
            actual_titles = [p['title'] for e in (actual or {}).get('errors',[]) for p in e['problems']]
            if parsed.returncode != 0 or titles != ['NAMING ERROR'] or actual_titles != ['NAMING ERROR']:
                failures.append(name + ': qualified keyword syntax and naming phase')
        if name.startswith('hex-sentinel-'):
            parsed = subprocess.run([str(rust), 'parse', str(project/entry)], env=env, capture_output=True, text=True, timeout=30)
            if parsed.returncode != 0 or titles != ['NAMING ERROR']:
                failures.append(name + ': hexadecimal sentinel token boundary')
        if name == 'number-exponent-tail':
            parsed = subprocess.run([str(rust), 'parse', str(project/entry)], env=env, capture_output=True, text=True, timeout=30)
            if parsed.returncode != 0 or titles != ['TOO MANY ARGS']:
                failures.append(name + ': exponent token boundary')
        if name == 'operator-double-dot':
            parsed = subprocess.run([str(rust), 'parse', str(project/entry)], env=env, capture_output=True, text=True, timeout=30)
            # Both parse the operator; only name resolution rejects its absent definition.
            if parsed.returncode != 0 or titles != ['UNKNOWN EXPORT']:
                failures.append(name + ': operator syntax acceptance')
        compared = len(titles) == 1 and titles[0] in ['MISSING ARGUMENT', 'UNFINISHED ANONYMOUS FUNCTION', 'LET PROBLEM', 'PROBLEM IN DEFINITION', 'MISSING COLON?', 'EXPECTING DEFINITION', 'NAME MISMATCH', 'UNFINISHED DEFINITION', 'UNFINISHED LET', 'MISSING EXPRESSION', 'EXPECTING MODULE NAME', 'MODULE NAME MISSING', 'UNFINISHED MODULE DECLARATION', 'UNFINISHED PORT MODULE DECLARATION', 'UNFINISHED EXPOSING', 'PROBLEM EXPOSING CUSTOM TYPE VARIANTS', 'PROBLEM IN EXPOSING', 'RESERVED SYMBOL', 'RESERVED WORD', 'UNEXPECTED SYMBOL', 'EXPECTING IMPORT NAME', 'EXPECTING IMPORT ALIAS', 'UNFINISHED IMPORT', 'WEIRD DECLARATION', 'STRAY PARENTHESIS', 'STRAY SQUARE BRACKET', 'STRAY CURLY BRACE', 'UNEXPECTED CAPITAL LETTER', 'NO TABS', 'ENDLESS COMMENT', 'BAD INFIX', 'BAD MODULE DECLARATION', 'UNEXPECTED PORTS', 'NO PORTS', 'PACKAGES CANNOT HAVE PORTS', 'INVALID EFFECT MODULE', 'WEIRD NUMBER', 'WEIRD HEXIDECIMAL', 'LEADING ZEROS', 'ENDLESS STRING', 'MISSING SINGLE QUOTE', 'NEEDS DOUBLE QUOTES', 'UNKNOWN ESCAPE', 'BAD UNICODE ESCAPE', 'MISSING ARROW', 'UNFINISHED CASE', 'UNFINISHED IF', 'WEIRD ELSE BRANCH', 'UNEXPECTED OPERATOR', 'UNFINISHED PARENTHESES', 'UNFINISHED TUPLE', 'UNFINISHED OPERATOR FUNCTION', 'EXPECTING RECORD ACCESSOR', 'UNFINISHED LIST', 'UNFINISHED RECORD', 'NEED MORE INDENTATION', 'PROBLEM IN RECORD', 'EXTRA COMMA', 'UNFINISHED TUPLE PATTERN', 'PROBLEM IN PATTERN', 'UNFINISHED LIST PATTERN', 'UNFINISHED RECORD PATTERN', 'UNEXPECTED NAME', 'UNFINISHED PATTERN', 'UNEXPECTED PATTERN']
        official_terminal = rust_terminal = official_ansi = rust_ansi = None
        if runs[0].returncode != runs[1].returncode:
            failures.append(name + ': exit code')
        if compared:
            if expected != actual:
                failures.append(name + ': complete JSON')
            terminals = [subprocess.run([str(binary),*command],cwd=project,env=env,capture_output=True,text=True,timeout=30) for binary in [args.elm.resolve(),rust]]
            official_terminal, rust_terminal = [run.stderr for run in terminals]
            if official_terminal != rust_terminal:
                failures.append(name + ': plain terminal')
            official_code, official_ansi = run_terminal([str(args.elm.resolve()), *command], project, env)
            rust_code, rust_ansi = run_terminal([str(rust), *command], project, env)
            if official_code != rust_code or official_ansi != rust_ansi:
                failures.append(name + ': ANSI terminal')
        results.append({'case':name,'compared_full':compared,'official':expected,'official_raw_invalid_json':raw_reference_json,'rust':actual,'official_ansi':official_ansi,'rust_ansi':rust_ansi,'official_terminal':official_terminal,'rust_terminal':rust_terminal})
        # Each case owns its project. Keep captured diagnostics in the report,
        # but release compiler caches now instead of accumulating 1,569 projects
        # until TemporaryDirectory exits (which can exhaust the /tmp quota).
        shutil.rmtree(project)
report = {'scope':__doc__,'case_prefixes':args.case_prefix,'cases':len(results),'full_comparisons':sum(x['compared_full'] for x in results),'reference_invalid_json':reference_invalid_json,'failures':failures,'results':results,'compiler_sha256':hashlib.sha256(rust.read_bytes()).hexdigest()}
if args.report:
    args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({key:report[key] for key in ['cases','full_comparisons','failures']}))
raise SystemExit(bool(failures))
