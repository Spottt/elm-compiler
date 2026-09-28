port module Main exposing (main)
import Platform

type Msg = Received Int
port incoming : (Int -> msg) -> Sub msg
port outgoing : Int -> Cmd msg
main : Program { start : Int } Int Msg
main = Platform.worker
    { init = \flags -> (flags.start, Cmd.none)
    , update = \(Received value) model -> (model, outgoing (if booleanChecks value then value + model else -999))
    , subscriptions = \_ -> incoming Received
    }

booleanChecks : Int -> Bool
booleanChecks value =
    not (False && explode value)
        && (True || explode value)
        && not ((&&) False (explode value))
        && ((||) True (explode value))
        && (True && (value == value))
        && (False || (value == value))

explode : Int -> Bool
explode value =
    explode (value + 1)
