//! One shared mapping for Elm record fields. JSON keys are never renamed.
use crate::{
    ast::{Expr, Pattern, Type},
    kernel,
    project::Graph,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Fields(Option<BTreeMap<String, String>>);

impl Fields {
    pub fn name(&self, name: &str) -> Result<String, String> {
        match &self.0 {
            None => Ok(crate::js_names::field(name)),
            Some(names) => names
                .get(name)
                .cloned()
                .ok_or_else(|| format!("record field missing from production mapping: {name}")),
        }
    }

    pub fn production(graph: &Graph) -> Result<Self, String> {
        let mut counts = BTreeMap::<String, usize>::new();
        let mut add = |name: &str| {
            *counts.entry(name.into()).or_default() += 1;
        };
        for module in &graph.modules {
            if module.kernel {
                for chunk in kernel::parse(&module.source)?.chunks {
                    if let kernel::Chunk::ElmField(name) = chunk {
                        add(name);
                    }
                }
                continue;
            }
            let ast = crate::parser::parse(&module.source)
                .map_err(|e| format!("{}: {e}", module.path.display()))?;
            for expression in &ast.expressions {
                match &expression.kind {
                    Expr::Literal(crate::lexer::Kind::Shader, raw) => {
                        let shader = crate::shader::Shader::parse(raw)?;
                        for name in shader.attributes.keys().chain(shader.uniforms.keys()) {
                            add(name);
                        }
                    }
                    Expr::Access(_, name) | Expr::Accessor(name) => add(name),
                    Expr::Record { fields, .. } => {
                        for (name, _) in fields {
                            add(name);
                        }
                    }
                    _ => {}
                }
            }
            for pattern in &ast.patterns {
                if let Pattern::Record(names) = &pattern.kind {
                    for name in names {
                        add(name);
                    }
                }
            }
            for ty in &ast.types {
                if let Type::Record { fields, .. } = &ty.kind {
                    for (name, _) in fields {
                        add(name);
                    }
                }
            }
        }
        let mut fields: Vec<_> = counts.into_iter().collect();
        // More frequent fields get shorter names. Lexical tie-breaking keeps
        // output deterministic across traversal order and hash randomization.
        fields.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Ok(Self(Some(
            fields
                .into_iter()
                .enumerate()
                .map(|(index, (name, _))| (name, crate::js_names::kernel_field(index)))
                .collect(),
        )))
    }
}
