//! Share equal closed cache types without merging quantified variables.
use serde_json::{Value, json};
use std::collections::HashMap;
fn rewrite(node: &Value, map: &mut impl FnMut(usize) -> Option<usize>) -> Option<Value> {
    let mut node = node.clone();
    let mut reference = |value: &mut Value| -> Option<()> {
        *value = json!(map(usize::try_from(value.as_u64()?).ok()?)?);
        Some(())
    };
    match node.get(0)?.as_str()? {
        "variable" | "rigid" | "unit" => {}
        "function" => {
            reference(node.get_mut(1)?)?;
            reference(node.get_mut(2)?)?;
        }
        "tuple" => {
            for value in node.get_mut(1)?.as_array_mut()? {
                reference(value)?;
            }
        }
        "named" | "alias" => {
            let alias = node[0] == "alias";
            for value in node.get_mut(2)?.as_array_mut()? {
                reference(value)?;
            }
            if alias {
                reference(node.get_mut(3)?)?;
            }
        }
        "record" => {
            for value in node.get_mut(1)?.as_object_mut()?.values_mut() {
                reference(value)?;
            }
            if !node.get(2)?.is_null() {
                reference(node.get_mut(2)?)?;
            }
        }
        _ => return None,
    }
    Some(node)
}
pub fn compact(graph: &Value) -> Option<Value> {
    // Named source variables and record order are diagnostic/debug metadata.
    if graph.get("variable_names").is_some() || graph.get("record_field_order").is_some() {
        return None;
    }
    let nodes = graph.get("nodes")?.as_array()?;
    let mut state = vec![0u8; nodes.len()];
    let mut remap = vec![0usize; nodes.len()];
    let mut closed = vec![false; nodes.len()];
    let mut output = Vec::new();
    let mut intern = HashMap::<Vec<u8>, usize>::new();
    for root in 0..nodes.len() {
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            if *state.get(id)? == 2 {
                continue;
            }
            if !finish {
                if state[id] == 1 {
                    return None;
                } // retain cyclic graphs unchanged
                state[id] = 1;
                pending.push((id, true));
                rewrite(&nodes[id], &mut |child| {
                    pending.push((child, false));
                    Some(child)
                })?;
                continue;
            }
            let mut is_closed = !matches!(nodes[id][0].as_str()?, "variable" | "rigid");
            let node = rewrite(&nodes[id], &mut |child| {
                if *state.get(child)? != 2 {
                    return None;
                }
                is_closed &= closed[child];
                Some(remap[child])
            })?;
            let key = if is_closed {
                Some(serde_json::to_vec(&node).ok()?)
            } else {
                None
            };
            let next = key
                .as_ref()
                .and_then(|key| intern.get(key))
                .copied()
                .unwrap_or_else(|| {
                    let next = output.len();
                    output.push(node);
                    if let Some(key) = key {
                        intern.insert(key, next);
                    }
                    next
                });
            remap[id] = next;
            closed[id] = is_closed;
            state[id] = 2;
        }
    }
    let roots: Option<Vec<_>> = graph
        .get("roots")?
        .as_array()?
        .iter()
        .map(|v| remap.get(usize::try_from(v.as_u64()?).ok()?).copied())
        .collect();
    let mut compacted = json!({"version":graph.get("version")?,"roots":roots?});
    compacted["nodes"] = Value::Array(output);
    Some(compacted)
}
