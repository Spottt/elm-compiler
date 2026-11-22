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
