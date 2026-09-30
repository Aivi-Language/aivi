#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeRecordField {
    pub label: Box<str>,
    pub value: RuntimeValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeMapEntry {
    pub key: RuntimeValue,
    pub value: RuntimeValue,
}

/// Ordered runtime map storage backed by an `IndexMap`.
///
/// Lookups use an expected constant number of hash-table probes, plus the cost
/// of hashing and comparing the key. Insertion order is preserved as written,
/// satisfying the display and serialisation invariant that `{b: 2, a: 1}`
/// prints with `b` before `a`.
///
/// Equality and hashing ignore insertion order. Hashing visits every entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeMap(IndexMap<RuntimeValue, RuntimeValue>);

// One immutable process seed is shared by every map, so equal maps use the same
// entry digest regardless of insertion order, construction, or caller hasher.
// This hash-table digest is never serialized or used as a persistent fingerprint.
static RUNTIME_MAP_ENTRY_HASHER: std::sync::LazyLock<foldhash::quality::RandomState> =
    std::sync::LazyLock::new(foldhash::quality::RandomState::default);

impl RuntimeMap {
    /// Build a map from a list of entries, preserving insertion order.
    ///
    /// Duplicate keys are silently overwritten with the last value, matching
    /// the evaluation-time behaviour for map literals with repeated keys.
    pub fn from_entries(entries: Vec<RuntimeMapEntry>) -> Self {
        let mut map = IndexMap::with_capacity(entries.len());
        for entry in entries {
            map.insert(entry.key, entry.value);
        }
        Self(map)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over key-value pairs in insertion order.
    pub fn iter(&self) -> indexmap::map::Iter<'_, RuntimeValue, RuntimeValue> {
        self.0.iter()
    }

    /// Look up a value using expected O(1) probes, plus key hashing/equality.
    pub fn get(&self, key: &RuntimeValue) -> Option<&RuntimeValue> {
        self.0.get(key)
    }
}

impl std::hash::Hash for RuntimeMap {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::BuildHasher;

        // Hash associated pairs independently, then combine commutatively.
        // Use the quality variant because this sum needs well-distributed entry
        // digests. A fixed or per-map seed would permit precomputed collisions
        // or violate the requirement that equal maps hash equally, respectively.
        let entry_hasher = &*RUNTIME_MAP_ENTRY_HASHER;
        let mut entries = 0_u64;
        for entry in &self.0 {
            entries = entries.wrapping_add(entry_hasher.hash_one(entry));
        }
        self.len().hash(state);
        entries.hash(state);
    }
}

impl<'a> IntoIterator for &'a RuntimeMap {
    type Item = (&'a RuntimeValue, &'a RuntimeValue);
    type IntoIter = indexmap::map::Iter<'a, RuntimeValue, RuntimeValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod map_hash_tests {
    use super::*;
    use std::hash::{DefaultHasher, Hasher};

    fn map(entries: &[(i64, i64)]) -> RuntimeMap {
        RuntimeMap::from_entries(
            entries
                .iter()
                .map(|&(key, value)| RuntimeMapEntry {
                    key: RuntimeValue::Int(key),
                    value: RuntimeValue::Int(value),
                })
                .collect(),
        )
    }

    fn hash(value: &impl Hash) -> u64 {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }

    #[test]
    fn equal_maps_hash_equally_for_every_insertion_order() {
        let expected = map(&[(1, 10), (2, 20), (3, 30)]);
        for order in [
            [1, 2, 3],
            [1, 3, 2],
            [2, 1, 3],
            [2, 3, 1],
            [3, 1, 2],
            [3, 2, 1],
        ] {
            let actual = map(&order.map(|key| (key, key * 10)));
            assert_eq!(expected, actual);
            assert_eq!(hash(&expected), hash(&actual));
            assert_eq!(
                actual
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>(),
                order.map(RuntimeValue::Int)
            );
        }
    }

    #[test]
    fn reordered_nested_map_keys_support_lookup_and_last_value_replacement() {
        let first = RuntimeValue::Map(map(&[(1, 10), (2, 20)]));
        let reordered = RuntimeValue::Map(map(&[(2, 20), (1, 10)]));
        let different = RuntimeValue::Map(map(&[(3, 30), (4, 40)]));
        for (first, reordered, different) in [
            (first.clone(), reordered.clone(), different.clone()),
            (
                RuntimeValue::List(vec![first]),
                RuntimeValue::List(vec![reordered]),
                RuntimeValue::List(vec![different]),
            ),
        ] {
            // Singleton lookups bypass hashing in IndexMap. Use two distinct
            // keys to exercise the hash/equality contract during lookup.
            let outer = RuntimeMap::from_entries(vec![
                RuntimeMapEntry {
                    key: first.clone(),
                    value: RuntimeValue::Int(1),
                },
                RuntimeMapEntry {
                    key: different.clone(),
                    value: RuntimeValue::Int(99),
                },
            ]);
            assert_eq!(outer.get(&reordered), Some(&RuntimeValue::Int(1)));
            let outer = RuntimeMap::from_entries(vec![
                RuntimeMapEntry {
                    key: first.clone(),
                    value: RuntimeValue::Int(1),
                },
                RuntimeMapEntry {
                    key: different.clone(),
                    value: RuntimeValue::Int(99),
                },
                RuntimeMapEntry {
                    key: reordered,
                    value: RuntimeValue::Int(2),
                },
            ]);
            assert_eq!(outer.len(), 2);
            assert_eq!(outer.get(&first), Some(&RuntimeValue::Int(2)));
            assert_eq!(outer.get(&different), Some(&RuntimeValue::Int(99)));
            assert_eq!(
                format!("{:?}", outer.iter().next().unwrap().0),
                format!("{first:?}")
            );
        }
    }

    #[test]
    fn map_hash_tracks_final_values_and_key_value_associations() {
        let replaced = map(&[(1, 99), (2, 20), (1, 10)]);
        let expected = map(&[(2, 20), (1, 10)]);
        assert_eq!(replaced, expected);
        assert_eq!(hash(&replaced), hash(&expected));
        // Guard against hashing keys and values as two independent multisets.
        assert_ne!(hash(&expected), hash(&map(&[(1, 20), (2, 10)])));
        assert_ne!(hash(&RuntimeMap::default()), hash(&map(&[(1, 10)])));

        let wrap = |value| {
            RuntimeMap::from_entries(vec![RuntimeMapEntry {
                key: RuntimeValue::Unit,
                value: RuntimeValue::Map(value),
            }])
        };
        assert_eq!(hash(&wrap(replaced)), hash(&wrap(expected)));
    }

    #[test]
    fn hashing_preserves_equality_debug_and_serialization() {
        let map = map(&[(1, 10), (2, 20)]);
        let cold_clone = map.clone();
        let bytes = postcard::to_stdvec(&map).unwrap();
        let debug = format!("{map:?}");
        let digest = hash(&map);
        assert_eq!(map, cold_clone);
        assert_eq!(debug, format!("{map:?}"));
        assert_eq!(bytes, postcard::to_stdvec(&map).unwrap());
        // Preserve the original newtype's wire representation exactly.
        assert_eq!(bytes, postcard::to_stdvec(&map.0).unwrap());
        let warm_clone = map.clone();
        let decoded: RuntimeMap = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, map);
        assert_eq!(hash(&decoded), digest);
        assert_eq!(hash(&cold_clone), digest);
        assert_eq!(hash(&warm_clone), digest);
    }

    #[test]
    fn concurrent_hashing_uses_one_order_independent_digest() {
        let map = map(&[(1, 10), (2, 20), (3, 30)]);
        let expected = hash(&self::map(&[(3, 30), (1, 10), (2, 20)]));
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    barrier.wait();
                    assert_eq!(hash(&map), expected);
                });
            }
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeSumValue {
    /// The HIR item that defines this sum type.
    ///
    /// # Staleness after recompilation
    ///
    /// This `HirItemId` is assigned at decode/evaluation time and refers to the item identity in
    /// the HIR layer at that moment. After any recompilation that changes HIR structure — for
    /// example, adding, removing, or reordering type definitions — the numeric `HirItemId` may
    /// point at a different item or become invalid entirely. Runtime sum values that were produced
    /// before such a recompile must be re-decoded against the new HIR before they are used in any
    /// context that dispatches on `item` (e.g. structural equality, variant dispatch, serialization).
    pub item: HirItemId,
    pub type_name: Box<str>,
    pub variant_name: Box<str>,
    pub fields: Vec<RuntimeValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RuntimeConstructor {
    Some,
    Ok,
    Err,
    Valid,
    Invalid,
}

/// Database leaves in the common task carrier. The worker owns execution;
/// callbacks receive the query rows or commit unit value after success.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RuntimeDbTaskPlan {
    Query(RuntimeDbQueryPlan),
    Commit(RuntimeDbCommitPlan),
}

/// Path-backed database identity extracted from the surface `Connection` record.
///
/// The `database` text must already be normalized so equality and change invalidation use the same
/// canonical key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeDbConnection {
    pub database: Box<str>,
}

/// One SQL statement plus its bound arguments.
///
/// Argument order is significant and preserves the lowering order so later execution can bind
/// placeholders deterministically without re-inspecting source syntax.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeDbStatement {
    pub sql: Box<str>,
    pub arguments: Vec<RuntimeValue>,
}

/// Read-only DB work.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeDbQueryPlan {
    pub connection: RuntimeDbConnection,
    pub statement: RuntimeDbStatement,
}

/// Transactional DB work whose successful commit must invalidate explicit table keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeDbCommitPlan {
    pub connection: RuntimeDbConnection,
    pub statements: Vec<RuntimeDbStatement>,
    pub changed_tables: BTreeSet<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeNamedValue {
    pub name: Box<str>,
    pub value: RuntimeValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RuntimeCustomCapabilityCommandPlan {
    pub provider_key: Box<str>,
    pub command: Box<str>,
    pub provider_arguments: Box<[RuntimeNamedValue]>,
    pub options: Box<[RuntimeNamedValue]>,
    pub arguments: Box<[RuntimeNamedValue]>,
}

impl fmt::Display for RuntimeDbTaskPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query(plan) => write!(f, "db.{plan}"),
            Self::Commit(plan) => write!(f, "db.{plan}"),
        }
    }
}

impl fmt::Display for RuntimeDbConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "db.connection({})", self.database)
    }
}

impl fmt::Display for RuntimeDbStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sql({}", self.sql)?;
        if !self.arguments.is_empty() {
            f.write_str("; args: [")?;
            for (index, argument) in self.arguments.iter().enumerate() {
                if index > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{argument}")?;
            }
            f.write_str("]")?;
        }
        f.write_str(")")
    }
}

impl fmt::Display for RuntimeDbQueryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "query({}, {})", self.connection, self.statement)
    }
}

impl fmt::Display for RuntimeDbCommitPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "commit({}, [", self.connection)?;
        for (index, statement) in self.statements.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{statement}")?;
        }
        f.write_str("]; changes: [")?;
        for (index, table) in self.changed_tables.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            f.write_str(table)?;
        }
        f.write_str("])")
    }
}

#[derive(Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RuntimeTaskPlan {
    Pure {
        value: Box<RuntimeValue>,
    },
    RandomInt {
        low: i64,
        high: i64,
    },
    RandomBytes {
        count: i64,
    },
    StdoutWrite {
        text: Box<str>,
    },
    StderrWrite {
        text: Box<str>,
    },
    FsWriteText {
        path: Box<str>,
        text: Box<str>,
    },
    FsWriteBytes {
        path: Box<str>,
        bytes: Box<[u8]>,
    },
    FsCreateDirAll {
        path: Box<str>,
    },
    FsDeleteFile {
        path: Box<str>,
    },
    FsReadText {
        path: Box<str>,
    },
    FsReadDir {
        path: Box<str>,
    },
    FsExists {
        path: Box<str>,
    },
    FsReadBytes {
        path: Box<str>,
    },
    FsRename {
        from: Box<str>,
        to: Box<str>,
    },
    FsCopy {
        from: Box<str>,
        to: Box<str>,
    },
    FsDeleteDir {
        path: Box<str>,
    },
    JsonValidate {
        json: Box<str>,
    },
    JsonGet {
        json: Box<str>,
        key: Box<str>,
    },
    JsonAt {
        json: Box<str>,
        index: i64,
    },
    JsonKeys {
        json: Box<str>,
    },
    JsonPretty {
        json: Box<str>,
    },
    JsonMinify {
        json: Box<str>,
    },
    // Time task plans
    TimeNowMs,
    TimeMonotonicMs,
    // Env task plans
    EnvGet {
        name: Box<str>,
    },
    EnvList {
        prefix: Box<str>,
    },
    // Log task plans
    LogEmit {
        level: Box<str>,
        message: Box<str>,
    },
    LogEmitContext {
        level: Box<str>,
        message: Box<str>,
        context: Box<[(Box<str>, Box<str>)]>,
    },
    // Random float task plan
    RandomFloat,
    // Regex task plans
    RegexIsMatch {
        pattern: Box<str>,
        text: Box<str>,
    },
    RegexFind {
        pattern: Box<str>,
        text: Box<str>,
    },
    RegexFindText {
        pattern: Box<str>,
        text: Box<str>,
    },
    RegexFindAll {
        pattern: Box<str>,
        text: Box<str>,
    },
    RegexReplace {
        pattern: Box<str>,
        replacement: Box<str>,
        text: Box<str>,
    },
    RegexReplaceAll {
        pattern: Box<str>,
        replacement: Box<str>,
        text: Box<str>,
    },
    // HTTP task plans (run on worker thread via ureq)
    HttpGet {
        url: Box<str>,
    },
    HttpGetBytes {
        url: Box<str>,
    },
    HttpGetStatus {
        url: Box<str>,
    },
    HttpPost {
        url: Box<str>,
        content_type: Box<str>,
        body: Box<str>,
    },
    HttpPut {
        url: Box<str>,
        content_type: Box<str>,
        body: Box<str>,
    },
    HttpDelete {
        url: Box<str>,
    },
    HttpHead {
        url: Box<str>,
    },
    HttpPostJson {
        url: Box<str>,
        body: Box<str>,
    },
    DbusCall {
        destination: Box<str>,
        path: Box<str>,
        interface: Box<str>,
        member: Box<str>,
        body: Box<[RuntimeValue]>,
        bus: Box<str>,
        address: Box<str>,
    },
    SecretLookup {
        service: Box<str>,
        attributes: Box<[(Box<str>, Box<str>)]>,
    },
    SecretStore {
        service: Box<str>,
        label: Box<str>,
        attributes: Box<[(Box<str>, Box<str>)]>,
        value: Box<str>,
    },
    SecretDelete {
        service: Box<str>,
        attributes: Box<[(Box<str>, Box<str>)]>,
    },
    NotificationSend {
        app_name: Box<str>,
        notification: Box<RuntimeValue>,
        bus: Box<str>,
        address: Box<str>,
    },
    NotificationClose {
        app_name: Box<str>,
        id: i64,
        bus: Box<str>,
        address: Box<str>,
    },
    AuthPkce {
        config: Box<RuntimeValue>,
    },
    AuthRefresh {
        config: Box<RuntimeValue>,
        refresh_token: Box<str>,
    },
    CustomCapabilityCommand(RuntimeCustomCapabilityCommandPlan),
    /// Deferred map: execute `inner`, then apply `function` to the result and wrap in `Pure`.
    Map {
        function: Box<RuntimeValue>,
        inner: Box<RuntimeTaskPlan>,
    },
    /// Deferred apply: execute both tasks, apply the function-result to the value-result.
    Apply {
        function_task: Box<RuntimeTaskPlan>,
        value_task: Box<RuntimeTaskPlan>,
    },
    /// Deferred chain: execute `inner`, apply `function` (which returns a `Task`), execute that.
    Chain {
        function: Box<RuntimeValue>,
        inner: Box<RuntimeTaskPlan>,
    },
    /// Deferred join: execute `outer` (which produces a `Task`), then execute that inner task.
    Join {
        outer: Box<RuntimeTaskPlan>,
    },
    Database(RuntimeDbTaskPlan),
}

impl fmt::Display for RuntimeTaskPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(plan) => plan.fmt(f),
            Self::Pure { value } => write!(f, "pure({value})"),
            Self::RandomInt { low, high } => write!(f, "randomInt({low}, {high})"),
            Self::RandomBytes { count } => write!(f, "randomBytes({count})"),
            Self::StdoutWrite { text } => write!(f, "stdoutWrite({text})"),
            Self::StderrWrite { text } => write!(f, "stderrWrite({text})"),
            Self::FsWriteText { path, .. } => write!(f, "writeText({path})"),
            Self::FsWriteBytes { path, .. } => write!(f, "writeBytes({path})"),
            Self::FsCreateDirAll { path } => write!(f, "createDirAll({path})"),
            Self::FsDeleteFile { path } => write!(f, "deleteFile({path})"),
            Self::FsReadText { path } => write!(f, "readText({path})"),
            Self::FsReadDir { path } => write!(f, "readDir({path})"),
            Self::FsExists { path } => write!(f, "exists({path})"),
            Self::FsReadBytes { path } => write!(f, "readBytes({path})"),
            Self::FsRename { from, to } => write!(f, "rename({from}, {to})"),
            Self::FsCopy { from, to } => write!(f, "copy({from}, {to})"),
            Self::FsDeleteDir { path } => write!(f, "deleteDir({path})"),
            Self::JsonValidate { json } => write!(f, "json.validate({json})"),
            Self::JsonGet { json, key } => write!(f, "json.get({json}, {key})"),
            Self::JsonAt { json, index } => write!(f, "json.at({json}, {index})"),
            Self::JsonKeys { json } => write!(f, "json.keys({json})"),
            Self::JsonPretty { json } => write!(f, "json.pretty({json})"),
            Self::JsonMinify { json } => write!(f, "json.minify({json})"),
            Self::TimeNowMs => f.write_str("time.nowMs"),
            Self::TimeMonotonicMs => f.write_str("time.monotonicMs"),
            Self::EnvGet { name } => write!(f, "env.get({name})"),
            Self::EnvList { prefix } => write!(f, "env.list({prefix})"),
            Self::LogEmit { level, message } => write!(f, "log.emit({level}, {message})"),
            Self::LogEmitContext { level, message, .. } => {
                write!(f, "log.emitContext({level}, {message})")
            }
            Self::RandomFloat => f.write_str("random.randomFloat"),
            Self::RegexIsMatch { pattern, text } => write!(f, "regex.isMatch({pattern}, {text})"),
            Self::RegexFind { pattern, text } => write!(f, "regex.find({pattern}, {text})"),
            Self::RegexFindText { pattern, text } => {
                write!(f, "regex.findText({pattern}, {text})")
            }
            Self::RegexFindAll { pattern, text } => write!(f, "regex.findAll({pattern}, {text})"),
            Self::RegexReplace {
                pattern,
                replacement,
                text,
            } => {
                write!(f, "regex.replace({pattern}, {replacement}, {text})")
            }
            Self::RegexReplaceAll {
                pattern,
                replacement,
                text,
            } => {
                write!(f, "regex.replaceAll({pattern}, {replacement}, {text})")
            }
            Self::HttpGet { url } => write!(f, "http.get({url})"),
            Self::HttpGetBytes { url } => write!(f, "http.getBytes({url})"),
            Self::HttpGetStatus { url } => write!(f, "http.getStatus({url})"),
            Self::HttpPost { url, .. } => write!(f, "http.post({url})"),
            Self::HttpPut { url, .. } => write!(f, "http.put({url})"),
            Self::HttpDelete { url } => write!(f, "http.delete({url})"),
            Self::HttpHead { url } => write!(f, "http.head({url})"),
            Self::HttpPostJson { url, .. } => write!(f, "http.postJson({url})"),
            Self::DbusCall {
                destination,
                path,
                interface,
                member,
                ..
            } => write!(f, "dbus.call({destination}, {path}, {interface}, {member})"),
            Self::SecretLookup { service, .. } => write!(f, "secret.lookup({service})"),
            Self::SecretStore { service, label, .. } => {
                write!(f, "secret.store({service}, {label})")
            }
            Self::SecretDelete { service, .. } => write!(f, "secret.delete({service})"),
            Self::NotificationSend { app_name, .. } => {
                write!(f, "notifications.send({app_name})")
            }
            Self::NotificationClose { app_name, id, .. } => {
                write!(f, "notifications.close({app_name}, {id})")
            }
            Self::AuthPkce { .. } => f.write_str("auth.pkce(...)"),
            Self::AuthRefresh { .. } => f.write_str("auth.refresh(...)"),
            Self::CustomCapabilityCommand(plan) => {
                write!(f, "{}.{}", plan.provider_key, plan.command)
            }
            Self::Map { .. } => f.write_str("task.map(...)"),
            Self::Apply { .. } => f.write_str("task.apply(...)"),
            Self::Chain { .. } => f.write_str("task.chain(...)"),
            Self::Join { .. } => f.write_str("task.join(...)"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RuntimeCallable {
    ItemBody {
        item: ItemId,
        kernel: KernelId,
        parameters: Vec<LayoutId>,
        bound_arguments: Vec<RuntimeValue>,
    },
    BuiltinConstructor {
        constructor: RuntimeConstructor,
        bound_arguments: Vec<RuntimeValue>,
    },
    SumConstructor {
        handle: SumConstructorHandle,
        bound_arguments: Vec<RuntimeValue>,
    },
    DomainMember {
        handle: DomainMemberHandle,
        parameters: Vec<LayoutId>,
        result: LayoutId,
        bound_arguments: Vec<RuntimeValue>,
    },
    BuiltinClassMember {
        intrinsic: BuiltinClassMemberIntrinsic,
        bound_arguments: Vec<RuntimeValue>,
    },
    IntrinsicValue {
        value: IntrinsicValue,
        bound_arguments: Vec<RuntimeValue>,
    },
}

#[derive(Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    Float(RuntimeFloat),
    Decimal(RuntimeDecimal),
    BigInt(RuntimeBigInt),
    Text(Box<str>),
    Bytes(Box<[u8]>),
    Tuple(Vec<RuntimeValue>),
    List(Vec<RuntimeValue>),
    // Runtime maps preserve source entry order while using order-independent
    // equality and hashing, including when a map is itself used as a key.
    Map(RuntimeMap),
    Set(Vec<RuntimeValue>),
    Record(Vec<RuntimeRecordField>),
    Sum(RuntimeSumValue),
    OptionNone,
    OptionSome(Box<RuntimeValue>),
    ResultOk(Box<RuntimeValue>),
    ResultErr(Box<RuntimeValue>),
    ValidationValid(Box<RuntimeValue>),
    ValidationInvalid(Box<RuntimeValue>),
    Signal(Box<RuntimeValue>),
    Task(RuntimeTaskPlan),
    SuffixedInteger { raw: Box<str>, suffix: Box<str> },
    Callable(RuntimeCallable),
}

impl RuntimeValue {
    /// Leave scalar destruction to the owner; detach recursive trees before the
    /// owner replaces or clears a slot. This preserves the common scalar path.
    pub(crate) fn discard_tree_in_place(&mut self) {
        if !runtime_value_is_scalar(self) {
            std::mem::replace(self, Self::Unit).discard();
        }
    }

    /// Release an owned value without recursively dropping its value or task tree.
    /// Use this when abandoning deeply nested pending work. This does not execute effects
    /// and does not change the ordinary value representation or its automatic destructor.
    pub fn discard(self) {
        // Scalar snapshots have no tree to traverse. Keep their normal drop
        // allocation-free, including the empty sentinel used by boundary owners.
        if matches!(
            self,
            Self::Unit
                | Self::Bool(_)
                | Self::Int(_)
                | Self::Float(_)
                | Self::Decimal(_)
                | Self::BigInt(_)
                | Self::Text(_)
                | Self::Bytes(_)
                | Self::OptionNone
                | Self::SuffixedInteger { .. }
        ) {
            return;
        }
        let mut pending = match self {
            Self::Tuple(values) | Self::List(values) | Self::Set(values) => values,
            Self::Map(ref map)
                if map.iter().all(|(key, value)| {
                    runtime_value_is_scalar(key) && runtime_value_is_scalar(value)
                }) =>
            {
                return;
            }
            other => vec![other],
        };
        while let Some(value) = pending.pop() {
            match value {
                Self::Tuple(values) | Self::List(values) | Self::Set(values) => {
                    pending.extend(values)
                }
                Self::Map(values) => {
                    for (key, value) in values.0 {
                        pending.push(key);
                        pending.push(value);
                    }
                }
                Self::Record(fields) => pending.extend(fields.into_iter().map(|field| field.value)),
                Self::Sum(value) => pending.extend(value.fields),
                Self::OptionSome(value)
                | Self::ResultOk(value)
                | Self::ResultErr(value)
                | Self::ValidationValid(value)
                | Self::ValidationInvalid(value)
                | Self::Signal(value) => pending.push(*value),
                Self::Callable(callable) => {
                    let (RuntimeCallable::ItemBody {
                        bound_arguments, ..
                    }
                    | RuntimeCallable::BuiltinConstructor {
                        bound_arguments, ..
                    }
                    | RuntimeCallable::SumConstructor {
                        bound_arguments, ..
                    }
                    | RuntimeCallable::DomainMember {
                        bound_arguments, ..
                    }
                    | RuntimeCallable::BuiltinClassMember {
                        bound_arguments, ..
                    }
                    | RuntimeCallable::IntrinsicValue {
                        bound_arguments, ..
                    }) = callable;
                    pending.extend(bound_arguments);
                }
                Self::Task(plan) => match plan {
                    RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Query(plan)) => {
                        pending.extend(plan.statement.arguments)
                    }
                    RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Commit(plan)) => {
                        for statement in plan.statements {
                            pending.extend(statement.arguments);
                        }
                    }
                    RuntimeTaskPlan::Pure { value } => pending.push(*value),
                    RuntimeTaskPlan::Map { function, inner }
                    | RuntimeTaskPlan::Chain { function, inner } => {
                        pending.push(*function);
                        pending.push(Self::Task(*inner));
                    }
                    RuntimeTaskPlan::Apply {
                        function_task,
                        value_task,
                    } => {
                        pending.push(Self::Task(*function_task));
                        pending.push(Self::Task(*value_task));
                    }
                    RuntimeTaskPlan::Join { outer } => pending.push(Self::Task(*outer)),
                    RuntimeTaskPlan::DbusCall { body, .. } => pending.extend(body),
                    RuntimeTaskPlan::NotificationSend { notification, .. } => {
                        pending.push(*notification)
                    }
                    RuntimeTaskPlan::AuthPkce { config }
                    | RuntimeTaskPlan::AuthRefresh { config, .. } => pending.push(*config),
                    RuntimeTaskPlan::CustomCapabilityCommand(plan) => {
                        for argument in plan
                            .provider_arguments
                            .into_iter()
                            .chain(plan.options)
                            .chain(plan.arguments)
                        {
                            pending.push(argument.value);
                        }
                    }
                    RuntimeTaskPlan::RandomInt { .. }
                    | RuntimeTaskPlan::RandomBytes { .. }
                    | RuntimeTaskPlan::StdoutWrite { .. }
                    | RuntimeTaskPlan::StderrWrite { .. }
                    | RuntimeTaskPlan::FsWriteText { .. }
                    | RuntimeTaskPlan::FsWriteBytes { .. }
                    | RuntimeTaskPlan::FsCreateDirAll { .. }
                    | RuntimeTaskPlan::FsDeleteFile { .. }
                    | RuntimeTaskPlan::FsReadText { .. }
                    | RuntimeTaskPlan::FsReadDir { .. }
                    | RuntimeTaskPlan::FsExists { .. }
                    | RuntimeTaskPlan::FsReadBytes { .. }
                    | RuntimeTaskPlan::FsRename { .. }
                    | RuntimeTaskPlan::FsCopy { .. }
                    | RuntimeTaskPlan::FsDeleteDir { .. }
                    | RuntimeTaskPlan::JsonValidate { .. }
                    | RuntimeTaskPlan::JsonGet { .. }
                    | RuntimeTaskPlan::JsonAt { .. }
                    | RuntimeTaskPlan::JsonKeys { .. }
                    | RuntimeTaskPlan::JsonPretty { .. }
                    | RuntimeTaskPlan::JsonMinify { .. }
                    | RuntimeTaskPlan::EnvGet { .. }
                    | RuntimeTaskPlan::EnvList { .. }
                    | RuntimeTaskPlan::LogEmit { .. }
                    | RuntimeTaskPlan::LogEmitContext { .. }
                    | RuntimeTaskPlan::RegexIsMatch { .. }
                    | RuntimeTaskPlan::RegexFind { .. }
                    | RuntimeTaskPlan::RegexFindText { .. }
                    | RuntimeTaskPlan::RegexFindAll { .. }
                    | RuntimeTaskPlan::RegexReplace { .. }
                    | RuntimeTaskPlan::RegexReplaceAll { .. }
                    | RuntimeTaskPlan::HttpGet { .. }
                    | RuntimeTaskPlan::HttpGetBytes { .. }
                    | RuntimeTaskPlan::HttpGetStatus { .. }
                    | RuntimeTaskPlan::HttpPost { .. }
                    | RuntimeTaskPlan::HttpPut { .. }
                    | RuntimeTaskPlan::HttpDelete { .. }
                    | RuntimeTaskPlan::HttpHead { .. }
                    | RuntimeTaskPlan::HttpPostJson { .. }
                    | RuntimeTaskPlan::SecretLookup { .. }
                    | RuntimeTaskPlan::SecretStore { .. }
                    | RuntimeTaskPlan::SecretDelete { .. }
                    | RuntimeTaskPlan::NotificationClose { .. }
                    | RuntimeTaskPlan::TimeNowMs
                    | RuntimeTaskPlan::TimeMonotonicMs
                    | RuntimeTaskPlan::RandomFloat => {}
                },
                Self::Unit
                | Self::Bool(_)
                | Self::Int(_)
                | Self::Float(_)
                | Self::Decimal(_)
                | Self::BigInt(_)
                | Self::Text(_)
                | Self::Bytes(_)
                | Self::OptionNone
                | Self::SuffixedInteger { .. } => {}
            }
        }
    }
}

#[cfg(test)]
mod disposal_tests {
    use super::*;

    #[test]
    fn discard_releases_deep_containers_tasks_and_captures_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut value = RuntimeValue::Unit;
                for index in 0..20_000 {
                    value = match index % 6 {
                        0 => RuntimeValue::List(vec![value]),
                        1 => RuntimeValue::OptionSome(Box::new(value)),
                        2 => RuntimeValue::Task(RuntimeTaskPlan::Pure {
                            value: Box::new(value),
                        }),
                        3 => RuntimeValue::Record(vec![RuntimeRecordField {
                            label: "value".into(),
                            value,
                        }]),
                        4 => RuntimeValue::Callable(RuntimeCallable::BuiltinConstructor {
                            constructor: RuntimeConstructor::Some,
                            bound_arguments: vec![value],
                        }),
                        _ => RuntimeValue::Map(RuntimeMap::from_entries(vec![RuntimeMapEntry {
                            key: RuntimeValue::Unit,
                            value,
                        }])),
                    };
                }
                value.discard();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

/// Explicit snapshot used when runtime values cross GTK/worker/FFI boundaries.
///
/// Snapshots own independent value trees. Copying and releasing a deep tree use
/// explicit worklists; no ordinary value address is assumed to remain stable.
/// The private disposal flag records whether copying left any recursive child
/// work. Shallow copies keep Rust's normal destructor; all other owners and
/// decoded snapshots conservatively use iterative release.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct DetachedRuntimeValue {
    value: RuntimeValue,
    #[serde(skip, default = "snapshot_requires_iterative_drop")]
    iterative_drop: bool,
}

const fn snapshot_requires_iterative_drop() -> bool {
    true
}

impl DetachedRuntimeValue {
    pub const fn unit() -> Self {
        Self {
            value: RuntimeValue::Unit,
            iterative_drop: false,
        }
    }

    #[inline]
    pub fn from_runtime_copy(value: &RuntimeValue) -> Self {
        clone_runtime_snapshot(value)
    }

    pub fn from_runtime_owned(value: RuntimeValue) -> Self {
        Self {
            value,
            iterative_drop: true,
        }
    }

    pub const fn as_runtime(&self) -> &RuntimeValue {
        &self.value
    }

    pub fn to_runtime(&self) -> RuntimeValue {
        self.value.clone()
    }

    pub fn into_runtime(mut self) -> RuntimeValue {
        std::mem::replace(&mut self.value, RuntimeValue::Unit)
    }
}

impl Drop for DetachedRuntimeValue {
    #[inline]
    fn drop(&mut self) {
        if self.iterative_drop && !runtime_value_is_scalar(&self.value) {
            std::mem::replace(&mut self.value, RuntimeValue::Unit).discard();
        }
    }
}

impl fmt::Debug for DetachedRuntimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DetachedRuntimeValue")
            .field(&self.value)
            .finish()
    }
}

impl PartialEq for DetachedRuntimeValue {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for DetachedRuntimeValue {}

impl PartialEq<RuntimeValue> for DetachedRuntimeValue {
    fn eq(&self, other: &RuntimeValue) -> bool {
        self.as_runtime() == other
    }
}

impl PartialEq<DetachedRuntimeValue> for RuntimeValue {
    fn eq(&self, other: &DetachedRuntimeValue) -> bool {
        self == other.as_runtime()
    }
}

impl fmt::Display for DetachedRuntimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_runtime().fmt(f)
    }
}

impl RuntimeValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value.to_f64()),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value.as_ref()),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(value) => Some(value.as_ref()),
            _ => None,
        }
    }

    fn write_display_text(&self, target: &mut impl fmt::Write) -> fmt::Result {
        let mut stack = vec![DisplayFrame::Value(self)];
        while let Some(frame) = stack.pop() {
            match frame {
                DisplayFrame::Value(value) => match value {
                    Self::Unit => target.write_str("()")?,
                    Self::Bool(true) => target.write_str("True")?,
                    Self::Bool(false) => target.write_str("False")?,
                    Self::Int(value) => write!(target, "{value}")?,
                    Self::Float(value) => write!(target, "{value}")?,
                    Self::Decimal(value) => write!(target, "{value}")?,
                    Self::BigInt(value) => write!(target, "{value}")?,
                    Self::Text(value) => target.write_str(value)?,
                    Self::Bytes(value) => write!(target, "<bytes:{}>", value.len())?,
                    Self::Tuple(elements) => {
                        push_delimited_values(&mut stack, elements, "(", ")");
                    }
                    Self::List(elements) => {
                        push_delimited_values(&mut stack, elements, "[", "]");
                    }
                    Self::Map(entries) => {
                        push_map_entries(&mut stack, entries);
                    }
                    Self::Set(elements) => {
                        push_delimited_values(&mut stack, elements, "#", "");
                    }
                    Self::Record(fields) => {
                        push_record_fields(&mut stack, fields);
                    }
                    Self::Sum(value) => {
                        push_sum_value(&mut stack, value);
                    }
                    Self::OptionNone => target.write_str("None")?,
                    Self::OptionSome(value) => {
                        stack.push(DisplayFrame::Value(value));
                        stack.push(DisplayFrame::StaticText("Some "));
                    }
                    Self::ResultOk(value) => {
                        stack.push(DisplayFrame::Value(value));
                        stack.push(DisplayFrame::StaticText("Ok "));
                    }
                    Self::ResultErr(value) => {
                        stack.push(DisplayFrame::Value(value));
                        stack.push(DisplayFrame::StaticText("Err "));
                    }
                    Self::ValidationValid(value) => {
                        stack.push(DisplayFrame::Value(value));
                        stack.push(DisplayFrame::StaticText("Valid "));
                    }
                    Self::ValidationInvalid(value) => {
                        stack.push(DisplayFrame::Value(value));
                        stack.push(DisplayFrame::StaticText("Invalid "));
                    }
                    Self::Signal(value) => {
                        stack.push(DisplayFrame::StaticText(")"));
                        stack.push(DisplayFrame::Value(value));
                        stack.push(DisplayFrame::StaticText("Signal("));
                    }
                    Self::Task(task) => write!(target, "<task {task}>")?,
                    Self::SuffixedInteger { raw, suffix } => write!(target, "{raw}{suffix}")?,
                    Self::Callable(callable) => match callable {
                        RuntimeCallable::ItemBody { item, .. } => {
                            write!(target, "<item-body item{item}>")?
                        }
                        RuntimeCallable::BuiltinConstructor { constructor, .. } => {
                            write!(target, "<constructor {constructor}>")?
                        }
                        RuntimeCallable::SumConstructor { handle, .. } => write!(
                            target,
                            "<constructor {}.{}>",
                            handle.type_name, handle.variant_name
                        )?,
                        RuntimeCallable::DomainMember { handle, .. } => write!(
                            target,
                            "<domain-member {}.{}>",
                            handle.domain_name, handle.member_name
                        )?,
                        RuntimeCallable::BuiltinClassMember { intrinsic, .. } => {
                            write!(target, "<builtin-class-member {intrinsic:?}>")?
                        }
                        RuntimeCallable::IntrinsicValue { value, .. } => {
                            write!(target, "<intrinsic-value {value}>")?
                        }
                    },
                },
                DisplayFrame::StaticText(text) => target.write_str(text)?,
                DisplayFrame::BorrowedText(text) => target.write_str(text)?,
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn display_text(&self) -> String {
        let mut rendered = String::new();
        self.write_display_text(&mut rendered)
            .expect("writing into a String should not fail");
        rendered
    }
}

impl fmt::Display for RuntimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_display_text(f)
    }
}

enum DisplayFrame<'a> {
    Value(&'a RuntimeValue),
    StaticText(&'static str),
    BorrowedText(&'a str),
}

fn push_delimited_values<'a>(
    stack: &mut Vec<DisplayFrame<'a>>,
    values: &'a [RuntimeValue],
    open: &'static str,
    close: &'static str,
) {
    stack.push(DisplayFrame::StaticText(close));
    for (index, value) in values.iter().enumerate().rev() {
        stack.push(DisplayFrame::Value(value));
        if index > 0 {
            stack.push(DisplayFrame::StaticText(", "));
        }
    }
    stack.push(DisplayFrame::StaticText(open));
}

fn push_map_entries<'a>(stack: &mut Vec<DisplayFrame<'a>>, entries: &'a RuntimeMap) {
    stack.push(DisplayFrame::StaticText("}"));
    for (index, (key, value)) in entries.iter().enumerate().rev() {
        stack.push(DisplayFrame::Value(value));
        stack.push(DisplayFrame::StaticText(": "));
        stack.push(DisplayFrame::Value(key));
        if index > 0 {
            stack.push(DisplayFrame::StaticText(", "));
        }
    }
    stack.push(DisplayFrame::StaticText("{"));
}

fn push_record_fields<'a>(stack: &mut Vec<DisplayFrame<'a>>, fields: &'a [RuntimeRecordField]) {
    stack.push(DisplayFrame::StaticText("}"));
    for (index, field) in fields.iter().enumerate().rev() {
        stack.push(DisplayFrame::Value(&field.value));
        stack.push(DisplayFrame::StaticText(": "));
        stack.push(DisplayFrame::BorrowedText(field.label.as_ref()));
        if index > 0 {
            stack.push(DisplayFrame::StaticText(", "));
        }
    }
    stack.push(DisplayFrame::StaticText("{"));
}

fn push_sum_value<'a>(stack: &mut Vec<DisplayFrame<'a>>, value: &'a RuntimeSumValue) {
    match value.fields.as_slice() {
        [] => stack.push(DisplayFrame::BorrowedText(value.variant_name.as_ref())),
        [field] => {
            stack.push(DisplayFrame::Value(field));
            stack.push(DisplayFrame::StaticText(" "));
            stack.push(DisplayFrame::BorrowedText(value.variant_name.as_ref()));
        }
        fields => {
            stack.push(DisplayFrame::StaticText(")"));
            for (index, field) in fields.iter().enumerate().rev() {
                stack.push(DisplayFrame::Value(field));
                if index > 0 {
                    stack.push(DisplayFrame::StaticText(", "));
                }
            }
            stack.push(DisplayFrame::StaticText("("));
            stack.push(DisplayFrame::BorrowedText(value.variant_name.as_ref()));
        }
    }
}

impl fmt::Display for RuntimeConstructor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Some => f.write_str("Some"),
            Self::Ok => f.write_str("Ok"),
            Self::Err => f.write_str("Err"),
            Self::Valid => f.write_str("Valid"),
            Self::Invalid => f.write_str("Invalid"),
        }
    }
}
