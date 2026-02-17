
fn diff_vnode(old: &Value, new: &Value, node_id: &str, out: &mut Vec<Value>) {
    if !same_vnode_shape(old, new) {
        let (html, _handlers) = render_vnode(new, node_id);
        out.push(Value::Constructor {
            name: "Replace".to_string(),
            args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
        });
        return;
    }

    match (old, new) {
        (Value::Constructor { name: on, args: oa }, Value::Constructor { name: nn, args: na })
            if on == "TextNode" && nn == "TextNode" && oa.len() == 1 && na.len() == 1 =>
        {
            let ot = match &oa[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            let nt = match &na[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            if ot != nt {
                out.push(Value::Constructor {
                    name: "SetText".to_string(),
                    args: vec![
                        Value::Text(node_id.to_string()),
                        Value::Text(nt.to_string()),
                    ],
                });
            }
        }
        (Value::Constructor { name: on, args: oa }, Value::Constructor { name: nn, args: na })
            if on == "Keyed" && nn == "Keyed" && oa.len() == 2 && na.len() == 2 =>
        {
            let ok = match &oa[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            let nk = match &na[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            if ok != nk {
                let (html, _handlers) = render_vnode(new, node_id);
                out.push(Value::Constructor {
                    name: "Replace".to_string(),
                    args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
                });
                return;
            }
            diff_vnode(&oa[1], &na[1], node_id, out);
        }
        (Value::Constructor { name: on, args: oa }, Value::Constructor { name: nn, args: na })
            if on == "Element" && nn == "Element" && oa.len() == 3 && na.len() == 3 =>
        {
            let otag = match &oa[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            let ntag = match &na[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            if otag != ntag {
                let (html, _handlers) = render_vnode(new, node_id);
                out.push(Value::Constructor {
                    name: "Replace".to_string(),
                    args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
                });
                return;
            }

            diff_attrs(&oa[1], &na[1], node_id, out);

            let oseg = child_segments(&oa[2]);
            let nseg = child_segments(&na[2]);
            if oseg != nseg {
                let (html, _handlers) = render_vnode(new, node_id);
                out.push(Value::Constructor {
                    name: "Replace".to_string(),
                    args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
                });
                return;
            }

            if let (Value::List(oc), Value::List(nc)) = (&oa[2], &na[2]) {
                for (idx, (ochild, nchild)) in oc.iter().zip(nc.iter()).enumerate() {
                    let seg = child_segment(nchild, idx);
                    let child_id = format!("{}/{}", node_id, seg);
                    diff_vnode(ochild, nchild, &child_id, out);
                }
            }
        }
        _ => {}
    }
}

fn child_segments(children: &Value) -> Vec<String> {
    let Value::List(items) = children else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .map(|(idx, child)| child_segment(child, idx))
        .collect()
}

fn same_vnode_shape(a: &Value, b: &Value) -> bool {
    matches!(
        (a, b),
        (
            Value::Constructor { name: an, args: aa },
            Value::Constructor { name: bn, args: ba }
        ) if an == bn && aa.len() == ba.len()
    )
}

fn attrs_to_map(attrs: &Value, node_id: &str) -> HashMap<String, String> {
    let mut state = RenderState {
        handlers: HashMap::new(),
    };
    let s = render_attrs(attrs, node_id, &mut state);
    let mut map = HashMap::new();
    let mut i = 0usize;
    let chars: Vec<char> = s.chars().collect();
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if key.is_empty() {
            break;
        }
        while i < chars.len() && chars[i] != '"' {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        i += 1;
        let vstart = i;
        while i < chars.len() && chars[i] != '"' {
            i += 1;
        }
        let value: String = chars[vstart..i].iter().collect();
        if i < chars.len() {
            i += 1;
        }
        map.insert(key.trim().to_string(), value);
    }
    map
}

fn diff_attrs(old: &Value, new: &Value, node_id: &str, out: &mut Vec<Value>) {
    let old_map = attrs_to_map(old, node_id);
    let new_map = attrs_to_map(new, node_id);

    let mut new_keys: Vec<&String> = new_map.keys().collect();
    new_keys.sort();
    for k in new_keys {
        let Some(v) = new_map.get(k) else {
            continue;
        };
        if old_map.get(k) != Some(v) {
            out.push(Value::Constructor {
                name: "SetAttr".to_string(),
                args: vec![
                    Value::Text(node_id.to_string()),
                    Value::Text(k.to_string()),
                    Value::Text(v.to_string()),
                ],
            });
        }
    }

    let mut old_keys: Vec<&String> = old_map.keys().collect();
    old_keys.sort();
    for k in old_keys {
        if !new_map.contains_key(k) {
            out.push(Value::Constructor {
                name: "RemoveAttr".to_string(),
                args: vec![Value::Text(node_id.to_string()), Value::Text(k.to_string())],
            });
        }
    }
}

fn patch_ops_to_json_text(value: &Value) -> Result<String, RuntimeError> {
    let json_value = patch_ops_to_json_value(value)?;
    serde_json::to_string(&json_value).map_err(|e| RuntimeError::Message(e.to_string()))
}

fn patch_ops_to_json_value(value: &Value) -> Result<serde_json::Value, RuntimeError> {
    let Value::List(items) = value else {
        return Err(RuntimeError::Message(
            "ui.patchToJson expects List PatchOp".to_string(),
        ));
    };
    let mut out = Vec::new();
    for item in items.iter() {
        let Value::Constructor { name, args } = item else {
            return Err(RuntimeError::Message(
                "ui.patchToJson expects PatchOp constructors".to_string(),
            ));
        };
        match (name.as_str(), args.as_slice()) {
            ("Replace", [Value::Text(id), Value::Text(html)]) => {
                out.push(serde_json::json!({"op":"replace","id":id,"html":html}));
            }
            ("SetText", [Value::Text(id), Value::Text(text)]) => {
                out.push(serde_json::json!({"op":"setText","id":id,"text":text}));
            }
            ("SetAttr", [Value::Text(id), Value::Text(name), Value::Text(value)]) => {
                out.push(serde_json::json!({"op":"setAttr","id":id,"name":name,"value":value}));
            }
            ("RemoveAttr", [Value::Text(id), Value::Text(name)]) => {
                out.push(serde_json::json!({"op":"removeAttr","id":id,"name":name}));
            }
            _ => {
                return Err(RuntimeError::Message(
                    "ui.patchToJson got invalid PatchOp".to_string(),
                ))
            }
        }
    }
    Ok(serde_json::Value::Array(out))
}
