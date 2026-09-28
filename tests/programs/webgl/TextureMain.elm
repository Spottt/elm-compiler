module TextureMain exposing (main)

import Browser
import Html exposing (Html)
import Html.Attributes exposing (height, width)
import Main exposing (mesh)
import Math.Vector2 exposing (Vec2)
import Math.Vector3 exposing (Vec3)
import Task
import WebGL exposing (Shader)
import WebGL.Texture as Texture exposing (Texture)


type Msg
    = Loaded (Result Texture.Error Texture)


type alias Uniforms =
    { texture : Texture }


type alias Varyings =
    { uv : Vec2 }


vertex : Shader { position : Vec3 } Uniforms Varyings
vertex =
    [glsl|
        attribute vec3 position;
        varying vec2 uv;
        void main() {
            gl_Position = vec4(position, 1.0);
            uv = (position.xy + 1.0) / 2.0;
        }
    |]


fragment : Shader {} Uniforms Varyings
fragment =
    [glsl|
        precision mediump float;
        uniform sampler2D texture;
        varying vec2 uv;
        void main() {
            gl_FragColor = texture2D(texture, uv);
        }
    |]


view : Maybe (Result Texture.Error Texture) -> Html Msg
view model =
    case model of
        Just (Ok texture) ->
            WebGL.toHtmlWith [ WebGL.preserveDrawingBuffer ] [ width 64, height 64 ]
                [ WebGL.entity vertex fragment mesh { texture = texture } ]

        Just (Err _) ->
            Html.text "Texture loading failed"

        Nothing ->
            Html.text "Loading texture"


main : Program () (Maybe (Result Texture.Error Texture)) Msg
main =
    Browser.element
        { init = \() -> ( Nothing, Task.attempt Loaded (Texture.load "/pixel.png") )
        , update = \(Loaded result) _ -> ( Just result, Cmd.none )
        , subscriptions = \_ -> Sub.none
        , view = view
        }
