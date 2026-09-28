module Main exposing (main, mesh)

import Html exposing (Html)
import Html.Attributes exposing (height, width)
import Math.Matrix4 exposing (Mat4, makeScale)
import Math.Vector2 exposing (Vec2, vec2)
import Math.Vector3 exposing (Vec3, vec3)
import Math.Vector4 exposing (Vec4, vec4)
import WebGL exposing (Mesh, Shader)


type alias Vertex =
    { position : Vec3 }


type alias Uniforms =
    { red : Float
    , green : Float
    , blue : Float
    , transform : Mat4
    , scale : Vec2
    , offset : Vec3
    , tint : Vec4
    , count : Int
    }


type alias Varyings =
    { color : Vec4 }


mesh : Mesh Vertex
mesh =
    WebGL.triangles
        [ ( { position = vec3 -1 -1 0 }
          , { position = vec3 3 -1 0 }
          , { position = vec3 -1 3 0 }
          )
        ]


vertex : Shader Vertex Uniforms Varyings
vertex =
    [glsl|
        attribute vec3 position;
        uniform mat4 transform;
        uniform vec2 scale;
        uniform vec3 offset;
        uniform vec4 tint;
        varying vec4 color;
        void main() {
            gl_Position = transform * vec4(position.xy * scale, position.z, 1.0) + vec4(offset, 0.0);
            color = vec4((position.xy + 1.0) / 2.0, 1.0, 1.0) * tint;
        }
    |]


fragment : Shader {} Uniforms Varyings
fragment =
    [glsl|
        precision mediump float;
        uniform float red;
        uniform float green;
        uniform float blue;
        uniform int count;
        varying vec4 color;
        void main() {
            gl_FragColor = count == 1 ? vec4(red, green, blue, 1.0) * color : vec4(0.0);
        }
    |]


main : Html msg
main =
    WebGL.toHtmlWith [ WebGL.preserveDrawingBuffer ] [ width 64, height 64 ]
        [ WebGL.entity vertex fragment mesh
            { red = 0.75
            , green = 0.25
            , blue = 0.5
            , transform = makeScale (vec3 2 2 1)
            , scale = vec2 0.5 0.5
            , offset = vec3 0 0 0
            , tint = vec4 1 1 1 1
            , count = 1
            }
        ]
