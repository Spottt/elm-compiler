//! GLSL source and the record interfaces exposed by Elm shader literals.
mod parser;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat4,
    Texture,
}
#[derive(Debug, Default)]
pub struct Shader {
    pub source: String,
    pub attributes: BTreeMap<String, Type>,
    pub uniforms: BTreeMap<String, Type>,
    pub varyings: BTreeMap<String, Type>,
}
impl Shader {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let source = raw
            .strip_prefix("[glsl|")
            .and_then(|s| s.strip_suffix("|]"))
            .ok_or("invalid shader delimiters")?;
        parser::parse(source)
    }
    pub fn emit(
        &self,
        field: &mut impl FnMut(&str) -> Result<String, String>,
    ) -> Result<String, String> {
        let mut translation = |fields: &BTreeMap<String, Type>| -> Result<String, String> {
            let pairs = fields
                .keys()
                .map(|name| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(&crate::js_names::field(name)).unwrap(),
                        serde_json::to_string(&field(name)?).unwrap()
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(format!("{{{}}}", pairs.join(",")))
        };
        Ok(format!(
            "{{src:{},attributes:{},uniforms:{}}}",
            serde_json::to_string(&self.source).unwrap(),
            translation(&self.attributes)?,
            translation(&self.uniforms)?
        ))
    }
}

// Reserved for future use by language-glsl 0.3.0, bundled with Elm 0.19.1.
fn is_legacy_reserved(word: &str) -> bool {
    matches!(
        word,
        "common"
            | "partition"
            | "active"
            | "asm"
            | "class"
            | "union"
            | "enum"
            | "typedef"
            | "template"
            | "this"
            | "packed"
            | "goto"
            | "inline"
            | "noinline"
            | "volatile"
            | "public"
            | "static"
            | "extern"
            | "external"
            | "interface"
            | "long"
            | "short"
            | "double"
            | "half"
            | "fixed"
            | "unsigned"
            | "superp"
            | "input"
            | "output"
            | "hvec2"
            | "hvec3"
            | "hvec4"
            | "dvec2"
            | "dvec3"
            | "dvec4"
            | "fvec2"
            | "fvec3"
            | "fvec4"
            | "sampler3DRect"
            | "filter"
            | "image1D"
            | "image2D"
            | "image3D"
            | "imageCube"
            | "iimage1D"
            | "iimage2D"
            | "iimage3D"
            | "iimageCube"
            | "uimage1D"
            | "uimage2D"
            | "uimage3D"
            | "uimageCube"
            | "image1DArray"
            | "image2DArray"
            | "iimage1DArray"
            | "iimage2DArray"
            | "uimage1DArray"
            | "uimage2DArray"
            | "image1DShadow"
            | "image2DShadow"
            | "image1DArrayShadow"
            | "image2DArrayShadow"
            | "imageBuffer"
            | "iimageBuffer"
            | "uimageBuffer"
            | "sizeof"
            | "cast"
            | "namespace"
            | "using"
            | "row_major"
    )
}
