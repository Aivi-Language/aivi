use std::collections::HashMap;

use crate::surface::Module;

pub fn ordered_module_names(modules: &[Module]) -> Vec<String> {
    ordered_module_indices(modules)
        .into_iter()
        .map(|idx| modules[idx].name.name.clone())
        .collect()
}

pub fn reverse_module_dependencies(modules: &[Module]) -> HashMap<String, Vec<String>> {
    let mut reverse = HashMap::new();
    for module in modules {
        reverse
            .entry(module.name.name.clone())
            .or_insert_with(Vec::new);
    }
    for module in modules {
        for use_decl in &module.uses {
            let dep = use_decl.module.name.clone();
            reverse
                .entry(dep)
                .or_insert_with(Vec::new)
                .push(module.name.name.clone());
        }
    }
    for dependents in reverse.values_mut() {
        dependents.sort();
        dependents.dedup();
    }
    reverse
}

pub(super) fn ordered_modules(modules: &[Module]) -> Vec<&Module> {
    let mut name_to_index = HashMap::new();
    for (idx, module) in modules.iter().enumerate() {
        name_to_index
            .entry(module.name.name.as_str())
            .or_insert(idx);
    }

    let mut indegree = vec![0usize; modules.len()];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); modules.len()];

    for (idx, module) in modules.iter().enumerate() {
        for use_decl in module.uses.iter() {
            let Some(&dep_idx) = name_to_index.get(use_decl.module.name.as_str()) else {
                continue;
            };
            if dep_idx == idx {
                continue;
            }
            edges[dep_idx].push(idx);
            indegree[idx] += 1;
        }
    }

    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &deg)| (deg == 0).then_some(idx))
        .collect();
    ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));

    let mut out = Vec::new();
    let mut processed = vec![false; modules.len()];
    while let Some(idx) = ready.first().copied() {
        ready.remove(0);
        if processed[idx] {
            continue;
        }
        processed[idx] = true;
        out.push(&modules[idx]);
        for &next in edges[idx].iter() {
            indegree[next] = indegree[next].saturating_sub(1);
            if indegree[next] == 0 && !processed[next] {
                ready.push(next);
                ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
            }
        }
    }

    let mut remaining: Vec<usize> = processed
        .iter()
        .enumerate()
        .filter_map(|(idx, done)| (!done).then_some(idx))
        .collect();
    remaining.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
    for idx in remaining {
        out.push(&modules[idx]);
    }

    out
}

pub(super) fn ordered_module_indices(modules: &[Module]) -> Vec<usize> {
    let mut name_to_index = HashMap::new();
    for (idx, module) in modules.iter().enumerate() {
        name_to_index
            .entry(module.name.name.as_str())
            .or_insert(idx);
    }

    let mut indegree = vec![0usize; modules.len()];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); modules.len()];

    for (idx, module) in modules.iter().enumerate() {
        for use_decl in module.uses.iter() {
            let Some(&dep_idx) = name_to_index.get(use_decl.module.name.as_str()) else {
                continue;
            };
            if dep_idx == idx {
                continue;
            }
            edges[dep_idx].push(idx);
            indegree[idx] += 1;
        }
    }

    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &deg)| (deg == 0).then_some(idx))
        .collect();
    ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));

    let mut out = Vec::new();
    let mut processed = vec![false; modules.len()];
    while let Some(idx) = ready.first().copied() {
        ready.remove(0);
        if processed[idx] {
            continue;
        }
        processed[idx] = true;
        out.push(idx);
        for &next in edges[idx].iter() {
            indegree[next] = indegree[next].saturating_sub(1);
            if indegree[next] == 0 && !processed[next] {
                ready.push(next);
                ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
            }
        }
    }

    let mut remaining: Vec<usize> = processed
        .iter()
        .enumerate()
        .filter_map(|(idx, done)| (!done).then_some(idx))
        .collect();
    remaining.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
    out.extend(remaining);
    out
}
