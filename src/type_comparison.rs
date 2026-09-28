//! Compare displayed type occurrences, rather than shared inference node IDs.
//! The same Int node can occur in both a matching and a mismatching record field.
use crate::{repl_type, type_localizer::Localizer};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn compare(graph: &Value, names: &Localizer) -> Option<Value> {
    compare_inner(graph, names, 0)
}
fn compare_inner(graph: &Value, names: &Localizer, depth: usize) -> Option<Value> {
    if depth > 128 {
        return None;
    }
    let nodes = graph["nodes"].as_array()?;
    let actual = graph["roots"][0].as_u64()? as usize;
    let expected = graph["roots"][1].as_u64()? as usize;
    let mut highlights = BTreeSet::new();
    let mut expected_highlights = BTreeSet::new();
    let mut expected_expansions = BTreeMap::new();
    let mut from_maybe = false;
    let mut to_list = false;
    let mut first_difference = None;
    let mut record_problem = None;
    let mut pending = vec![(actual, expected, Vec::new(), Vec::new())];
    while let Some((a, b, path, expected_path)) = pending.pop() {
        if path.len() > 1024 {
            return None;
        }
        let left = &nodes[a];
        let right = &nodes[b];
        if a == b || (left == right && !matches!(left[0].as_str(), Some("variable" | "rigid"))) {
            continue;
        }
        if (left[0] == "variable" && left[2] == "any")
            || (right[0] == "variable" && right[2] == "any")
        {
            continue;
        }
        if left[0] == "variable"
            && right[0] == "variable"
            && left[2] == "number"
            && right[2] == "number"
        {
            continue;
        }
        let number_matches = |variable: &Value, concrete: &Value| {
            variable[0] == "variable"
                && variable[2] == "number"
                && concrete[0] == "named"
                && concrete[1]
                    .as_str()
                    .and_then(|name| serde_json::from_str::<(String, String)>(name).ok())
                    .is_some_and(|(module, name)| {
                        module == "elm/core:Basics" && matches!(name.as_str(), "Int" | "Float")
                    })
        };
        if number_matches(left, right) || number_matches(right, left) {
            continue;
        }
        if left[0] == "named"
            && left[1]
                .as_str()
                .and_then(|name| serde_json::from_str::<(String, String)>(name).ok())
                .is_some_and(|(module, name)| module == "elm/core:Maybe" && name == "Maybe")
            && left[2].as_array().is_some_and(|args| args.len() == 1)
        {
            let mut inner = graph.clone();
            inner["roots"] = json!([left[2][0], b]);
            if compare_inner(&inner, names, depth + 1)?["similar"] == true {
                let mut head = path;
                head.push(usize::MAX);
                highlights.insert(head);
                if first_difference.is_none() {
                    from_maybe = true;
                }
                first_difference.get_or_insert((a, b));
                continue;
            }
        }
        if right[0] == "named"
            && right[1]
                .as_str()
                .and_then(|name| serde_json::from_str::<(String, String)>(name).ok())
                .is_some_and(|(module, name)| module == "elm/core:List" && name == "List")
            && right[2].as_array().is_some_and(|args| args.len() == 1)
        {
            let mut inner = graph.clone();
            inner["roots"] = json!([a, right[2][0]]);
            if compare_inner(&inner, names, depth + 1)?["similar"] == true {
                let mut head = expected_path;
                head.push(usize::MAX);
                expected_highlights.insert(head);
                if first_difference.is_none() {
                    to_list = true;
                }
                first_difference.get_or_insert((a, b));
                continue;
            }
        }
        let same_alias = left[0] == "alias" && right[0] == "alias" && left[1] == right[1];
        let aliased_records = if !same_alias && (left[0] == "alias" || right[0] == "alias") {
            let real_a = dealias(nodes, a)?;
            let real_b = dealias(nodes, b)?;
            (nodes[real_a][0] == "record" && nodes[real_b][0] == "record")
                .then_some((real_a, real_b))
        } else {
            None
        };
        let (display_a, display_b) = aliased_records.unwrap_or((a, b));
        if aliased_records.is_some() {
            // Elm compares the underlying fields while preserving the first
            // alias encountered as a wholly highlighted name.
            if left[0] == "alias" {
                highlights.insert(path.clone());
            }
            if right[0] == "alias" {
                if left[0] == "alias" {
                    // The first alias branch in Elm keeps only the left alias.
                    expected_expansions.insert(expected_path.clone(), display_b);
                } else {
                    expected_highlights.insert(expected_path.clone());
                }
            }
        }
        let (shape_a, children_a) = presentation(nodes, display_a)?;
        let (shape_b, children_b) = presentation(nodes, display_b)?;
        if shape_a[0] == "record" && shape_b[0] == "record" {
            let names_a = shape_a[1].as_array()?;
            let names_b = shape_b[1].as_array()?;
            let mut extra = Vec::new();
            let mut missing = Vec::new();
            for (index, name) in names_a.iter().enumerate() {
                if !names_b.contains(name) {
                    let mut field = path.clone();
                    field.extend([usize::MAX, index]);
                    highlights.insert(field);
                    extra.push(name.clone());
                }
            }
            for (index, name) in names_b.iter().enumerate() {
                if !names_a.contains(name) {
                    let mut field = expected_path.clone();
                    field.extend([usize::MAX, index]);
                    expected_highlights.insert(field);
                    missing.push(name.clone());
                }
            }
            let ext_a = (shape_a[2] == true).then(|| children_a[names_a.len()]);
            let ext_b = (shape_b[2] == true).then(|| children_b[names_b.len()]);
            let fixed_a = ext_a.is_none_or(|id| nodes[id][0] == "rigid");
            let fixed_b = ext_b.is_none_or(|id| nodes[id][0] == "rigid");
            let different_extensions = match (ext_a, ext_b) {
                (Some(x), Some(y)) => nodes[x][0] == "rigid" && nodes[y][0] == "rigid" && x != y,
                (Some(x), None) | (None, Some(x)) => nodes[x][0] == "rigid",
                _ => false,
            };
            if different_extensions {
                if let Some(x) = ext_a {
                    let mut child = path.clone();
                    child.push(names_a.len());
                    highlights.insert(child);
                    if let Some(y) = ext_b {
                        first_difference.get_or_insert((x, y));
                    }
                }
                if ext_b.is_some() {
                    let mut child = expected_path.clone();
                    child.push(names_b.len());
                    expected_highlights.insert(child);
                }
            }
            if !extra.is_empty() || !missing.is_empty() || different_extensions {
                let hint = match (fixed_a, fixed_b) {
                    (true, true) => {
                        json!({"extra":extra,"missing":missing,"possibilities":names_b})
                    }
                    (false, true) => json!({"extra":extra,"missing":[],"possibilities":names_b}),
                    (true, false) => json!({"extra":missing,"missing":[],"possibilities":names_a}),
                    (false, false) => json!({"extra":[],"missing":[],"possibilities":[]}),
                };
                record_problem.get_or_insert((a, b, hint));
            }
            for (i, name) in names_a.iter().enumerate().rev() {
                if let Some(j) = names_b.iter().position(|other| other == name) {
                    let mut child = path.clone();
                    child.push(i);
                    let mut expected_child = expected_path.clone();
                    expected_child.push(j);
                    pending.push((children_a[i], children_b[j], child, expected_child));
                }
            }
            continue;
        }
        if shape_a == shape_b && children_a.len() == children_b.len() && !children_a.is_empty() {
            for (index, (a, b)) in children_a.into_iter().zip(children_b).enumerate().rev() {
                let mut child = path.clone();
                child.push(index);
                let mut expected_child = expected_path.clone();
                expected_child.push(index);
                pending.push((a, b, child, expected_child));
            }
        } else {
            expected_highlights.insert(expected_path);
            highlights.insert(path);
            first_difference.get_or_insert((a, b));
        }
    }
    let record_hint = if first_difference.is_none() {
        if let Some((a, b, hint)) = record_problem {
            first_difference = Some((a, b));
            hint
        } else {
            Value::Null
        }
    } else {
        Value::Null
    };
    let actual_spans =
        repl_type::render_highlighted(graph, 0, names, &highlights, 76, &BTreeMap::new()).ok()?;
    let expected_spans = repl_type::render_highlighted(
        graph,
        1,
        names,
        &expected_highlights,
        76,
        &expected_expansions,
    )
    .ok()?;
    let mut leaf_graph = graph.clone();
    let (a, b) = first_difference.unwrap_or((actual, expected));
    leaf_graph["roots"] = json!([a, b]);
    let problem_name = |node: &Value| {
        if node[0] == "named" {
            node[1]
                .as_str()
                .and_then(|name| serde_json::from_str::<(String, String)>(name).ok())
                .map(|(_, name)| name)
        } else {
            None
        }
    };
    Some(
        json!({"record_hint":record_hint,"similar":first_difference.is_none(),"from_maybe":from_maybe,"to_list":to_list,"actual_spans":actual_spans,"expected_spans":expected_spans,
        "actual_problem":repl_type::render_width(&leaf_graph,0,names,76).ok()?,
        "expected_problem":repl_type::render_width(&leaf_graph,1,names,76).ok()?,
        "actual_problem_shape":nodes[a][0], "expected_problem_shape":nodes[b][0],
        "actual_problem_name":problem_name(&nodes[a]), "expected_problem_name":problem_name(&nodes[b]),
        "actual_constraint":nodes[a][2], "expected_constraint":nodes[b][2],
        "rigid":nodes[b][0] == "rigid", "actual_rigid":nodes[a][0] == "rigid",
        "actual_named_atom":nodes[a][0] == "named" && nodes[a][2].as_array().is_some_and(Vec::is_empty)}),
    )
}

fn dealias(nodes: &[Value], mut root: usize) -> Option<usize> {
    let mut seen = BTreeSet::new();
    while nodes.get(root)?[0] == "alias" {
        if !seen.insert(root) {
            return None;
        }
        root = nodes[root][3].as_u64()? as usize;
    }
    Some(root)
}

// Children are listed in the order in which the type renderer visits them.
fn presentation(nodes: &[Value], root: usize) -> Option<(Value, Vec<usize>)> {
    let node = nodes.get(root)?;
    let index = |v: &Value| v.as_u64().map(|v| v as usize);
    match node[0].as_str()? {
        "named" | "alias" => Some((
            json!([node[0], node[1]]),
            node[2]
                .as_array()?
                .iter()
                .map(index)
                .collect::<Option<_>>()?,
        )),
        "tuple" => Some((
            json!("tuple"),
            node[1]
                .as_array()?
                .iter()
                .map(index)
                .collect::<Option<_>>()?,
        )),
        "function" => {
            let mut children = Vec::new();
            let mut cursor = root;
            let mut seen = BTreeSet::new();
            while nodes[cursor][0] == "function" {
                if !seen.insert(cursor) {
                    return None;
                }
                children.push(index(&nodes[cursor][1])?);
                cursor = index(&nodes[cursor][2])?;
            }
            children.push(cursor);
            Some((json!("function"), children))
        }
        "record" => {
            let mut fields = BTreeMap::new();
            let mut cursor = root;
            let mut seen = BTreeSet::new();
            let extension = loop {
                if !seen.insert(cursor) {
                    return None;
                }
                let row = nodes.get(cursor)?;
                if row[0] == "alias" {
                    cursor = index(&row[3])?;
                    continue;
                }
                if row[0] != "record" {
                    break Some(cursor);
                }
                for (name, value) in row[1].as_object()? {
                    fields.insert(name, index(value)?);
                }
                if row[2].is_null() {
                    break None;
                }
                cursor = index(&row[2])?;
            };
            let mut children: Vec<_> = fields.values().copied().collect();
            children.extend(extension);
            Some((
                json!([
                    "record",
                    fields.keys().collect::<Vec<_>>(),
                    extension.is_some()
                ]),
                children,
            ))
        }
        _ => Some((node.clone(), vec![])),
    }
}
