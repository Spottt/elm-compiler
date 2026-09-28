module Main exposing (main)
import Browser
import Html exposing (button, div, text)
import Html.Events exposing (onClick)

type Msg = Increment

main = Browser.sandbox
    { init = 0
    , update = \Increment model -> model + 1
    , view = \model -> div []
        [ div [] [text "Version 1"]
        , div [] [text ("Counter: " ++ String.fromInt model)]
        , button [onClick Increment] [text "Increment"]
        ]
    }
