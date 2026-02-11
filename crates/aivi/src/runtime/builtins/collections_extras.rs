fn build_set_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "empty".to_string(),
        Value::Set(Arc::new(ImHashSet::new())),
    );
    fields.insert(
        "size".to_string(),
        builtin("set.size", 1, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.size")?;
            Ok(Value::Int(set.len() as i64))
        }),
    );
    fields.insert(
        "has".to_string(),
        builtin("set.has", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.has")?;
            let key = key_from_value(&args.pop().unwrap(), "set.has")?;
            Ok(Value::Bool(set.contains(&key)))
        }),
    );
    fields.insert(
        "insert".to_string(),
        builtin("set.insert", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.insert")?;
            let key = key_from_value(&args.pop().unwrap(), "set.insert")?;
            let mut out = (*set).clone();
            out.insert(key);
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("set.remove", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.remove")?;
            let key = key_from_value(&args.pop().unwrap(), "set.remove")?;
            let mut out = (*set).clone();
            out.remove(&key);
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "union".to_string(),
        builtin("set.union", 2, |mut args, _| {
            let right = expect_set(args.pop().unwrap(), "set.union")?;
            let left = expect_set(args.pop().unwrap(), "set.union")?;
            let out = (*left).clone().union((*right).clone());
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "intersection".to_string(),
        builtin("set.intersection", 2, |mut args, _| {
            let right = expect_set(args.pop().unwrap(), "set.intersection")?;
            let left = expect_set(args.pop().unwrap(), "set.intersection")?;
            let out = (*left).clone().intersection((*right).clone());
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "difference".to_string(),
        builtin("set.difference", 2, |mut args, _| {
            let right = expect_set(args.pop().unwrap(), "set.difference")?;
            let left = expect_set(args.pop().unwrap(), "set.difference")?;
            let out = (*left)
                .clone()
                .relative_complement((*right).clone());
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "fromList".to_string(),
        builtin("set.fromList", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "set.fromList")?;
            let mut out = ImHashSet::new();
            for item in items.iter() {
                let key = key_from_value(item, "set.fromList")?;
                out.insert(key);
            }
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "toList".to_string(),
        builtin("set.toList", 1, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.toList")?;
            let items = set.iter().map(|key| key.to_value()).collect();
            Ok(list_value(items))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_queue_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "empty".to_string(),
        Value::Queue(Arc::new(ImVector::new())),
    );
    fields.insert(
        "enqueue".to_string(),
        builtin("queue.enqueue", 2, |mut args, _| {
            let queue = expect_queue(args.pop().unwrap(), "queue.enqueue")?;
            let value = args.pop().unwrap();
            let mut out = (*queue).clone();
            out.push_back(value);
            Ok(Value::Queue(Arc::new(out)))
        }),
    );
    fields.insert(
        "dequeue".to_string(),
        builtin("queue.dequeue", 1, |mut args, _| {
            let queue = expect_queue(args.pop().unwrap(), "queue.dequeue")?;
            let mut out = (*queue).clone();
            match out.pop_front() {
                Some(value) => Ok(make_some(Value::Tuple(vec![
                    value,
                    Value::Queue(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peek".to_string(),
        builtin("queue.peek", 1, |mut args, _| {
            let queue = expect_queue(args.pop().unwrap(), "queue.peek")?;
            match queue.front() {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_deque_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "empty".to_string(),
        Value::Deque(Arc::new(ImVector::new())),
    );
    fields.insert(
        "pushFront".to_string(),
        builtin("deque.pushFront", 2, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.pushFront")?;
            let value = args.pop().unwrap();
            let mut out = (*deque).clone();
            out.push_front(value);
            Ok(Value::Deque(Arc::new(out)))
        }),
    );
    fields.insert(
        "pushBack".to_string(),
        builtin("deque.pushBack", 2, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.pushBack")?;
            let value = args.pop().unwrap();
            let mut out = (*deque).clone();
            out.push_back(value);
            Ok(Value::Deque(Arc::new(out)))
        }),
    );
    fields.insert(
        "popFront".to_string(),
        builtin("deque.popFront", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.popFront")?;
            let mut out = (*deque).clone();
            match out.pop_front() {
                Some(value) => Ok(make_some(Value::Tuple(vec![
                    value,
                    Value::Deque(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "popBack".to_string(),
        builtin("deque.popBack", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.popBack")?;
            let mut out = (*deque).clone();
            match out.pop_back() {
                Some(value) => Ok(make_some(Value::Tuple(vec![
                    value,
                    Value::Deque(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peekFront".to_string(),
        builtin("deque.peekFront", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.peekFront")?;
            match deque.front() {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peekBack".to_string(),
        builtin("deque.peekBack", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.peekBack")?;
            match deque.back() {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_heap_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "empty".to_string(),
        Value::Heap(Arc::new(BinaryHeap::new())),
    );
    fields.insert(
        "push".to_string(),
        builtin("heap.push", 2, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.push")?;
            let value = args.pop().unwrap();
            let key = key_from_value(&value, "heap.push")?;
            let mut out = (*heap).clone();
            out.push(Reverse(key));
            Ok(Value::Heap(Arc::new(out)))
        }),
    );
    fields.insert(
        "popMin".to_string(),
        builtin("heap.popMin", 1, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.popMin")?;
            let mut out = (*heap).clone();
            match out.pop() {
                Some(Reverse(value)) => Ok(make_some(Value::Tuple(vec![
                    value.to_value(),
                    Value::Heap(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peekMin".to_string(),
        builtin("heap.peekMin", 1, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.peekMin")?;
            match heap.peek() {
                Some(Reverse(value)) => Ok(make_some(value.to_value())),
                None => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_linalg_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "dot".to_string(),
        builtin("linalg.dot", 2, |mut args, _| {
            let (_, left) = vec_from_value(args.pop().unwrap(), "linalg.dot")?;
            let (_, right) = vec_from_value(args.pop().unwrap(), "linalg.dot")?;
            if left.len() != right.len() {
                return Err(RuntimeError::Message(
                    "linalg.dot expects vectors of equal size".to_string(),
                ));
            }
            let sum: f64 = left.iter().zip(right.iter()).map(|(a, b)| a * b).sum();
            Ok(Value::Float(sum))
        }),
    );
    fields.insert(
        "matMul".to_string(),
        builtin("linalg.matMul", 2, |mut args, _| {
            let (rows_b, cols_b, data_b) = mat_from_value(args.pop().unwrap(), "linalg.matMul")?;
            let (rows_a, cols_a, data_a) = mat_from_value(args.pop().unwrap(), "linalg.matMul")?;
            if cols_a != rows_b {
                return Err(RuntimeError::Message(
                    "linalg.matMul expects matching dimensions".to_string(),
                ));
            }
            let mut out = vec![0.0; (rows_a * cols_b) as usize];
            let rows_a_usize = rows_a as usize;
            let cols_a_usize = cols_a as usize;
            let cols_b_usize = cols_b as usize;
            for r in 0..rows_a_usize {
                for c in 0..cols_b_usize {
                    let mut acc = 0.0;
                    for k in 0..cols_a_usize {
                        let a = data_a[r * cols_a_usize + k];
                        let b = data_b[k * cols_b_usize + c];
                        acc += a * b;
                    }
                    out[r * cols_b_usize + c] = acc;
                }
            }
            Ok(mat_to_value(rows_a, cols_b, out))
        }),
    );
    fields.insert(
        "solve2x2".to_string(),
        builtin("linalg.solve2x2", 2, |mut args, _| {
            let (_, vec) = vec_from_value(args.pop().unwrap(), "linalg.solve2x2")?;
            let (rows, cols, mat) = mat_from_value(args.pop().unwrap(), "linalg.solve2x2")?;
            if rows != 2 || cols != 2 || vec.len() != 2 {
                return Err(RuntimeError::Message(
                    "linalg.solve2x2 expects 2x2 matrix and size-2 vector".to_string(),
                ));
            }
            let a = mat[0];
            let b = mat[1];
            let c = mat[2];
            let d = mat[3];
            let det = a * d - b * c;
            if det == 0.0 {
                return Err(RuntimeError::Message(
                    "linalg.solve2x2 determinant is zero".to_string(),
                ));
            }
            let x = (d * vec[0] - b * vec[1]) / det;
            let y = (-c * vec[0] + a * vec[1]) / det;
            Ok(vec_to_value(2, vec![x, y]))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_signal_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fft".to_string(),
        builtin("signal.fft", 1, |mut args, _| {
            let (samples, rate) = signal_from_value(args.pop().unwrap(), "signal.fft")?;
            if samples.is_empty() {
                return Ok(spectrum_to_value(Vec::new(), rate));
            }
            let mut planner = FftPlanner::new();
            let fft = planner.plan_fft_forward(samples.len());
            let mut buffer: Vec<FftComplex<f64>> = samples
                .into_iter()
                .map(|value| FftComplex::new(value, 0.0))
                .collect();
            fft.process(&mut buffer);
            Ok(spectrum_to_value(buffer, rate))
        }),
    );
    fields.insert(
        "ifft".to_string(),
        builtin("signal.ifft", 1, |mut args, _| {
            let (mut bins, rate) = spectrum_from_value(args.pop().unwrap(), "signal.ifft")?;
            if bins.is_empty() {
                return Ok(signal_to_value(Vec::new(), rate));
            }
            let mut planner = FftPlanner::new();
            let fft = planner.plan_fft_inverse(bins.len());
            fft.process(&mut bins);
            let scale = bins.len() as f64;
            let samples = bins
                .into_iter()
                .map(|value| value.re / scale)
                .collect();
            Ok(signal_to_value(samples, rate))
        }),
    );
    fields.insert(
        "windowHann".to_string(),
        builtin("signal.windowHann", 1, |mut args, _| {
            let (samples, rate) = signal_from_value(args.pop().unwrap(), "signal.windowHann")?;
            let len = samples.len();
            if len == 0 {
                return Ok(signal_to_value(samples, rate));
            }
            let denom = (len - 1) as f64;
            let mut out = Vec::with_capacity(len);
            for (i, value) in samples.into_iter().enumerate() {
                let phase = 2.0 * std::f64::consts::PI * (i as f64) / denom;
                let w = 0.5 * (1.0 - phase.cos());
                out.push(value * w);
            }
            Ok(signal_to_value(out, rate))
        }),
    );
    fields.insert(
        "normalize".to_string(),
        builtin("signal.normalize", 1, |mut args, _| {
            let (samples, rate) = signal_from_value(args.pop().unwrap(), "signal.normalize")?;
            let mut max = 0.0;
            for value in &samples {
                let abs = value.abs();
                if abs > max {
                    max = abs;
                }
            }
            if max == 0.0 {
                return Ok(signal_to_value(samples, rate));
            }
            let out = samples.into_iter().map(|value| value / max).collect();
            Ok(signal_to_value(out, rate))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_graph_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "addEdge".to_string(),
        builtin("graph.addEdge", 2, |mut args, _| {
            let edge_value = args.pop().unwrap();
            let graph_value = args.pop().unwrap();
            let (from, to, weight) = edge_from_value(edge_value, "graph.addEdge")?;
            let (mut nodes, mut edges) = graph_from_value(graph_value, "graph.addEdge")?;
            if !nodes.contains(&from) {
                nodes.push(from);
            }
            if !nodes.contains(&to) {
                nodes.push(to);
            }
            edges.push((from, to, weight));
            Ok(graph_to_value(nodes, edges))
        }),
    );
    fields.insert(
        "neighbors".to_string(),
        builtin("graph.neighbors", 2, |mut args, _| {
            let node = expect_int(args.pop().unwrap(), "graph.neighbors")?;
            let (_, edges) = graph_from_value(args.pop().unwrap(), "graph.neighbors")?;
            let neighbors: Vec<Value> = edges
                .iter()
                .filter_map(|(from, to, _)| {
                    if *from == node {
                        Some(Value::Int(*to))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::List(Arc::new(neighbors)))
        }),
    );
    fields.insert(
        "shortestPath".to_string(),
        builtin("graph.shortestPath", 3, |mut args, _| {
            let goal = expect_int(args.pop().unwrap(), "graph.shortestPath")?;
            let start = expect_int(args.pop().unwrap(), "graph.shortestPath")?;
            let (nodes, edges) = graph_from_value(args.pop().unwrap(), "graph.shortestPath")?;
            if start == goal {
                return Ok(Value::List(Arc::new(vec![Value::Int(start)])));
            }
            let mut adjacency: HashMap<i64, Vec<(i64, f64)>> = HashMap::new();
            for (from, to, weight) in edges {
                adjacency
                    .entry(from)
                    .or_default()
                    .push((to, weight));
            }
            for node in nodes {
                adjacency.entry(node).or_default();
            }
            let mut dist: HashMap<i64, f64> = HashMap::new();
            let mut prev: HashMap<i64, i64> = HashMap::new();
            let mut heap = BinaryHeap::new();
            dist.insert(start, 0.0);
            heap.push((Reverse(OrderedFloat(0.0)), start));
            while let Some((Reverse(OrderedFloat(cost)), node)) = heap.pop() {
                if cost > *dist.get(&node).unwrap_or(&f64::INFINITY) {
                    continue;
                }
                if node == goal {
                    break;
                }
                if let Some(neighbors) = adjacency.get(&node) {
                    for (next, weight) in neighbors {
                        let next_cost = cost + *weight;
                        let current = dist.get(next).copied().unwrap_or(f64::INFINITY);
                        if next_cost < current {
                            dist.insert(*next, next_cost);
                            prev.insert(*next, node);
                            heap.push((Reverse(OrderedFloat(next_cost)), *next));
                        }
                    }
                }
            }
            if !prev.contains_key(&goal) {
                return Ok(Value::List(Arc::new(Vec::new())));
            }
            let mut path = Vec::new();
            let mut current = goal;
            path.push(Value::Int(current));
            while current != start {
                match prev.get(&current) {
                    Some(node) => {
                        current = *node;
                        path.push(Value::Int(current));
                    }
                    None => return Ok(Value::List(Arc::new(Vec::new()))),
                }
            }
            path.reverse();
            Ok(Value::List(Arc::new(path)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_console_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "log".to_string(),
        builtin("console.log", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "println".to_string(),
        builtin("console.println", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "print".to_string(),
        builtin("console.print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    print!("{text}");
                    let mut out = std::io::stdout();
                    let _ = out.flush();
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "error".to_string(),
        builtin("console.error", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    eprintln!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "readLine".to_string(),
        builtin("console.readLine", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut buffer = String::new();
                    match std::io::stdin().read_line(&mut buffer) {
                        Ok(_) => Ok(make_ok(Value::Text(
                            buffer.trim_end_matches(&['\n', '\r'][..]).to_string(),
                        ))),
                        Err(err) => Ok(make_err(Value::Text(err.to_string()))),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn spawn_effect(
    id: usize,
    effect: Value,
    ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    sender: mpsc::Sender<(usize, Result<Value, RuntimeError>)>,
) {
    std::thread::spawn(move || {
        let mut runtime = Runtime::new(ctx, cancel);
        let result = runtime.run_effect_value(effect);
        let _ = sender.send((id, result));
    });
}
