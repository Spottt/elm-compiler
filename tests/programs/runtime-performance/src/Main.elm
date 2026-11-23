port module Main exposing (main)

{-| Runtime benchmark of the generated JavaScript. Each test stresses one kind
of generated code; `scripts/check_runtime_performance.py` times it with this
compiler's output and with official Elm's, and
`scripts/check_generated_shapes.py` compares the shape of both outputs.

`cases`, `records` and `decoding` come from
<https://github.com/Spottt/elm-compiler/issues/7>.

-}

import Dict
import Json.Decode as D


port done : Int -> Cmd msg


modulus : Int
modulus =
    1000000007



-- CASES: custom types, constructors, pattern matching


type Shape
    = Circle Int
    | Square Int
    | Rect Int Int
    | Tri Int Int Int


shapeOf : Int -> Shape
shapeOf i =
    case modBy 4 i of
        0 ->
            Circle i

        1 ->
            Square i

        2 ->
            Rect i 2

        _ ->
            Tri i 2 3


area : Shape -> Int
area s =
    case s of
        Circle r ->
            3 * r * r

        Square w ->
            w * w

        Rect w h ->
            w * h

        Tri a b c ->
            a + b + c


label : Maybe Int -> Int
label m =
    case m of
        Just n ->
            if n > 10 then
                n

            else
                0

        Nothing ->
            1


cases : Int -> Int -> Int
cases i acc =
    if i == 0 then
        acc

    else
        cases (i - 1) (modBy modulus (acc + area (shapeOf i) + label (Just i)))



-- RECORDS: alias constructors and field access


type alias Item =
    { a : Int, b : Int, c : Int, d : Int, e : Int }


records : Int -> Int -> Int
records i acc =
    if i == 0 then
        acc

    else
        let
            item =
                Item i (i + 1) (i + 2) (i + 3) (i + 4)
        in
        records (i - 1) (modBy modulus (acc + item.e))



-- CALLS: saturated calls, partial application, lambdas given to core


mix : Int -> Int -> Int -> Int
mix a b c =
    a + b * 3 - c


small : List Int
small =
    [ 1, 2, 3, 4 ]


calls : Int -> Int -> Int
calls i acc =
    if i == 0 then
        acc

    else
        let
            partial =
                mix i 2

            folded =
                List.foldl (\x sum -> sum + x * i) 0 small
        in
        calls (i - 1) (modBy modulus (mix acc i 1 + partial 3 + folded))



-- COMPARISONS: equality and ordering against literals


grade : Int -> Int
grade n =
    if n < 10 then
        1

    else if n <= 100 then
        2

    else if n >= 100000 then
        3

    else if n /= 5000 then
        4

    else
        5


kind : String -> Int
kind s =
    if s == "even" then
        2

    else if s == "odd" then
        1

    else
        0


parity : Int -> String
parity i =
    if modBy 2 i == 0 then
        "even"

    else
        "odd"


half : Int -> Int
half i =
    if toFloat i * 0.5 > 1000.5 then
        1

    else
        0


comparisons : Int -> Int -> Int
comparisons i acc =
    if i == 0 then
        acc

    else
        comparisons (i - 1) (modBy modulus (acc + grade i + kind (parity i) + half i))



-- LOCALS: multi-argument functions defined in a let, called saturated


locals : Int -> Int -> Int
locals i acc =
    if i == 0 then
        acc

    else
        let
            blend a b c =
                a * 3 + b - c

            scale a b =
                a * b + i
        in
        locals (i - 1) (modBy modulus (acc + blend i 2 1 + scale i 3 + blend acc 1 i))



-- FOLDS: a local function given to a core higher-order function


folds : Int -> Int
folds n =
    let
        step x sum =
            modBy modulus (sum + x * 3)

        keep x kept =
            if modBy 3 x == 0 then
                kept + 1

            else
                kept
    in
    List.foldl step 0 (List.range 1 n) + List.foldl keep 0 (List.range 1 n)



-- NESTED: let and case used as expressions, not in return position


weigh : Int -> Int
weigh n =
    n + 1


nested : Int -> Int -> Int
nested i acc =
    if i == 0 then
        acc

    else
        let
            total =
                acc
                    + (let
                        doubled =
                            i * 2
                       in
                       doubled + 1
                      )
                    + (case modBy 3 i of
                        0 ->
                            1

                        1 ->
                            i

                        _ ->
                            2
                      )
                    + weigh
                        (case shapeOf i of
                            Circle r ->
                                r

                            Square w ->
                                w + 1

                            Rect w h ->
                                w + h

                            Tri a b c ->
                                a + b + c
                        )
        in
        nested (i - 1) (modBy modulus total)



-- DICT: elm/core code compiled by the compiler under test


dict : Int -> Int
dict n =
    let
        keys =
            List.range 1 n

        filled =
            List.foldl (\i d -> Dict.insert (modBy 5003 (i * 7919)) i d) Dict.empty keys
    in
    modBy modulus (List.foldl (\i acc -> acc + Maybe.withDefault 0 (Dict.get i filled)) 0 keys)



-- STRINGS: append chains with literals


strings : Int -> Int -> Int
strings i acc =
    if i == 0 then
        acc

    else
        strings (i - 1)
            (modBy modulus (acc + String.length ("item-" ++ String.fromInt i ++ "/" ++ String.fromInt acc)))



-- DECODING: kernel decoders driving a record constructor


itemDecoder : D.Decoder Item
itemDecoder =
    D.map5 Item
        (D.field "a" D.int)
        (D.field "b" D.int)
        (D.field "c" D.int)
        (D.field "d" D.int)
        (D.field "e" D.int)


decoding : D.Value -> Int
decoding json =
    case D.decodeValue (D.list itemDecoder) json of
        Ok items ->
            List.foldl (\item acc -> acc + item.e) 0 items

        Err _ ->
            -1



-- MAIN


type alias Flags =
    { test : String, n : Int, json : D.Value }


run : Flags -> Int
run flags =
    case flags.test of
        "cases" ->
            cases flags.n 0

        "records" ->
            records flags.n 0

        "calls" ->
            calls flags.n 0

        "comparisons" ->
            comparisons flags.n 0

        "locals" ->
            locals flags.n 0

        "folds" ->
            folds flags.n

        "nested" ->
            nested flags.n 0

        "dict" ->
            dict flags.n

        "strings" ->
            strings flags.n 0

        _ ->
            decoding flags.json


main : Program Flags () ()
main =
    Platform.worker
        { init = \flags -> ( (), done (run flags) )
        , update = \_ model -> ( model, Cmd.none )
        , subscriptions = \_ -> Sub.none
        }
