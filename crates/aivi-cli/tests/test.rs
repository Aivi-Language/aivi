use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("aivi-{prefix}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary directory should be creatable");
        Self { path }
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("temporary parent directories should be creatable");
        }
        fs::write(&path, text).expect("temporary file should be writable");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn imported_authored_classes_execute_through_aliases_and_reexports() {
    let dir = TempDir::new("imported-authored-classes");
    dir.write(
        "classes.aivi",
        r#"
class Render A = { render : A -> Text }
type Render A => A -> Text
func show = value => render value
export Render
export show
class CompareF F = {
    with Functor F
    matches : Eq A => A -> F A -> F Bool
}
type (CompareF F, Eq A) => A -> F A -> F Bool
func matchAll = expected items => matches expected items
export CompareF
export matchAll
"#,
    );
    dir.write(
        "bridge.aivi",
        "use classes (Render as Display)\nexport Display\n",
    );
    dir.write(
        "carrier.aivi",
        r#"
use bridge (Display)
use classes (CompareF)
type Box A = Box A
instance Functor Box = { map = f box => box ||> Box a -> Box (f a) }
instance CompareF Box = { matches = expected box => box ||> Box a -> Box (a == expected) }
export Box
type Tag = Tag Text
instance Display Tag = { render = tag => tag ||> Tag text -> text }
type Tag -> Text
func display = tag => render tag
export Tag
export display
"#,
    );
    let path = dir.write(
        "main.aivi",
        r#"
use classes (Render as Rendering, CompareF, show, matchAll)
use bridge (Display)
use carrier (Tag, Box, display)
type Box Bool -> Bool
func unbox = box => box ||> Box value -> value
@test
value direct : Task Text Bool = pure (render (Tag "direct") == "direct")
@test
value generic : Task Text Bool = pure (show (Tag "generic") == "generic")
@test
value owner : Task Text Bool = pure (display (Tag "owner") == "owner")
@test
value methodLocal : Task Text Bool = pure (unbox (matches (Tag "yes") (Box (Tag "yes"))))
@test
value genericMethodLocal : Task Text Bool = pure (unbox (matchAll (Tag "yes") (Box (Tag "yes"))))
"#,
    );
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("5 passed; 0 failed; 5 total"), "{stdout}");
    }
}

#[test]
fn authored_standard_spellings_execute_imported_generic_dictionaries() {
    let dir = TempDir::new("authored-standard-dictionaries");
    for (module, class, proof) in [
        ("first", "Eq", "True"),
        ("second", "Eq", "False"),
        ("ordered", "Ord", "True"),
        ("setoid", "Setoid", "True"),
    ] {
        dir.write(
            &format!("{module}.aivi"),
            &format!(
                r#"
class {class} A = {{
    proof : A -> A -> Bool
    label : A -> Text
}}
instance {class} Int = {{
    proof = left right => {proof}
    label = value => "{module}"
}}
type {class} A => A -> A -> Bool
func accepts = left right => proof left right
type {class} A => A -> Text
func describe = value => label value
export {class}
export accepts
export describe
"#
            ),
        );
    }
    dir.write(
        "bridge.aivi",
        "use first (Eq as Forwarded)\nexport Forwarded\n",
    );
    let path = dir.write(
        "main.aivi",
        r#"
use bridge (Forwarded as First)
use first (accepts as firstAccepts, describe as firstDescribe)
use second (Eq as Second, accepts as secondAccepts)
use ordered (Ord as Ordered, accepts as orderedAccepts)
use setoid (Setoid as Relation, accepts as relationAccepts)
type (First A, Second A) => A -> A -> Bool
func differ = left right => firstAccepts left right != secondAccepts left right
@test
value first : Task Text Bool = pure (firstAccepts 1 2)
@test
value second : Task Text Bool = pure (secondAccepts 1 2 == False)
@test
value independent : Task Text Bool = pure (differ 1 2)
@test
value extraMember : Task Text Bool = pure (firstDescribe 1 == "first")
@test
value orderingName : Task Text Bool = pure (orderedAccepts 1 2)
@test
value relationName : Task Text Bool = pure (relationAccepts 1 2)
"#,
    );
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("6 passed; 0 failed; 6 total"), "{stdout}");
    }
}

#[test]
fn imported_equality_checks_closed_payloads_and_uses_standard_eq_instances() {
    let dir = TempDir::new("imported-closed-equality-execution");
    dir.write(
        "models.aivi",
        r#"
type Box A = Box A
type Custom = Custom Int
instance Eq Custom = { (==) = left right => True }
value left : Box (Option Int) = Box (Some 1)
value same : Box (Option Int) = Box (Some 1)
value different : Box (Option Int) = Box (Some 2)
value customLeft : Custom = Custom 1
value customRight : Custom = Custom 2
export Box
export Custom
export left
export same
export different
export customLeft
export customRight
"#,
    );
    let path = dir.write(
        "main.aivi",
        r#"
use models (Box as Original, Custom, left, same, different, customLeft, customRight)
use models (Box as Alias)
type Original (Option Int) -> Alias (Option Int) -> Bool
func equal = first second => first == second
@test
value matching : Task Text Bool = pure (equal left same)
@test
value differing : Task Text Bool = pure ((equal left different) == False)
@test
value authored : Task Text Bool = pure (customLeft == customRight)
@test
value authoredNegation : Task Text Bool = pure ((customLeft != customRight) == False)
"#,
    );
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .expect("test should run");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("4 passed; 0 failed; 4 total"), "{stdout}");
    }
}

#[test]
fn comparison_operators_preserve_imported_class_identity() {
    for class in ["Eq", "Equality"] {
        let library = format!(
            r#"
class {class} A = {{ (==) : A -> A -> Bool }}
class Ranking A = {{
    with {class} A
    compare : A -> A -> Ordering
}}
type Tag = Tag Int
instance {class} Tag = {{ (==) = left right => True }}
instance Ranking Tag = {{ compare = left right => Equal }}
type {class} A => A -> A -> Bool
func equal = left right => left == right
export {class}
export Ranking
export Tag
export equal
"#
        );
        let dir = TempDir::new("imported-comparison-evidence");
        dir.write("classes.aivi", &library);
        dir.write("bridge.aivi", &format!("use classes ({class} as Forwarded, Ranking as Rank)\nexport Forwarded\nexport Rank\n"));
        let path = dir.write(
            "main.aivi",
            r#"
use bridge (Forwarded as Matches, Rank as Order)
use classes (Tag, equal)
type Matches A => A -> A -> Bool
func forwarded = left right => left == right
type Order A => A -> A -> Bool
func ranked = left right => left <= right
@test
value generic : Task Text Bool = pure (equal (Tag 1) (Tag 2))
@test
value direct : Task Text Bool = pure (Tag 1 == Tag 2)
@test
value negated : Task Text Bool = pure ((Tag 1 != Tag 2) == False)
@test
value alias : Task Text Bool = pure (forwarded (Tag 1) (Tag 2))
@test
value ordering : Task Text Bool = pure ((Tag 1 < Tag 2) == False)
@test
value genericOrdering : Task Text Bool = pure (ranked (Tag 1) (Tag 2))
"#,
        );
        for _ in 0..2 {
            let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
                .arg("test")
                .arg(&path)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{class}: {stdout}\n{stderr}");
            assert!(stdout.contains("6 passed; 0 failed; 6 total"), "{stdout}");
        }
        let private = TempDir::new("private-comparison-projections");
        private.write("classes.aivi", &library);
        let path = private.write(
            "main.aivi",
            r#"
use classes (Tag, equal)
@test
value generic : Task Text Bool = pure (equal (Tag 1) (Tag 2))
@test
value ordinary : Task Text Bool = pure ((Tag 1 == Tag 2) == False)
"#,
        );
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{class}: {stdout}\n{stderr}");
        assert!(stdout.contains("2 passed; 0 failed; 2 total"), "{stdout}");
    }
}

#[test]
fn imported_classes_preserve_foreign_carriers_in_method_signatures() {
    let dir = TempDir::new("class-foreign-carriers");
    dir.write(
        "models.aivi",
        "type Box A = MkBox A\nexport Box\nexport MkBox\n",
    );
    dir.write(
        "facade.aivi",
        "use models (Box as Wrapped)\nexport Wrapped\n",
    );
    dir.write(
        "classes.aivi",
        r#"
use facade (Wrapped)
class Transform A = { transform : A -> Wrapped A }
type Transform A => A -> Wrapped A
func wrapped = value => transform value
export Transform
export wrapped
"#,
    );
    let path = dir.write(
        "main.aivi",
        r#"
use models (Box, MkBox)
use facade (Wrapped)
use classes (Transform, wrapped)
type Tag = Tag Text
instance Transform Tag = { transform = value => MkBox value }
type Box Tag -> Tag
func unbox = box => box ||> MkBox tag -> tag
type Tag -> Text
func label = tag => tag ||> Tag text -> text
@test
value direct : Task Text Bool = pure (label (unbox (transform (Tag "direct"))) == "direct")
@test
value generic : Task Text Bool = pure (label (unbox (wrapped (Tag "generic"))) == "generic")
"#,
    );
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("2 passed; 0 failed; 2 total"), "{stdout}");
    }
}

#[test]
fn generic_inline_callbacks_execute_with_lexical_dictionary_evidence() {
    let dir = TempDir::new("generic-inline-callbacks");
    dir.write(
        "callbacks.aivi",
        r#"
type Functor F => F Int -> F Int
func increment = value => map (n => n + 1) value
type Functor F => Int -> F Int -> F Int
func offset = amount value => map (n => n + amount) value
type Functor F => A -> F Int -> F A
func replace = captured value => map (n => captured) value
type (Functor F, Eq A) => A -> F A -> F Bool
func matches = captured value => map (n => n == captured) value
type Functor F => F (List Int) -> F (List Int)
func nested = value => map (items => map (n => n + 1) items) value
type Functor F => F Int -> F (Int -> Int)
func curried = value => map (n => m => n + m) value
export increment
export offset
export replace
export matches
export nested
export curried
"#,
    );
    let path = dir.write(
        "main.aivi",
        r#"
use callbacks (increment, offset, replace, matches, nested, curried)
type Box A = Box A
instance Functor Box = { map = f box => box ||> Box a -> Box (f a) }
type Tag = Tag Text
instance Eq Tag = {
    (==) = left right => (left, right) ||> (Tag a, Tag b) -> a == b
    (!=) = left right => (left, right) ||> (Tag a, Tag b) -> a != b
}
type (Int -> Int) -> Int
func invoke = f => f 10
type Box Int -> Bool
func boxIsThree = box => box ||> Box n -> n == 3
@test
value listInput : Task Text Bool = pure (increment [1, 2] == [2, 3])
@test
value optionInput : Task Text Bool = pure (increment (Some 1) == Some 2)
@test
value closedCapture : Task Text Bool = pure (offset 10 [1, 2] == [11, 12])
@test
value rigidCapture : Task Text Bool = pure (replace "kept" [1, 2] == ["kept", "kept"])
@test
value optionCapture : Task Text Bool = pure (replace "kept" (Some 1) == Some "kept")
@test
value eqCapture : Task Text Bool = pure (matches "yes" ["yes", "no"] == [True, False])
@test
value nestedCapture : Task Text Bool = pure (nested [[1, 2], [3]] == [[2, 3], [4]])
@test
value returnedLambda : Task Text Bool = pure (map invoke (curried [1, 2]) == [11, 12])
@test
value authoredFunctor : Task Text Bool = pure (boxIsThree (increment (Box 2)))
@test
value authoredEq : Task Text Bool = pure (matches (Tag "yes") [Tag "yes", Tag "no"] == [True, False])
"#,
    );
    // Independent compiler runs must preserve the same lexical dictionary scope.
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("10 passed; 0 failed; 10 total"), "{stdout}");
    }
}

#[test]
fn headless_traversal_forwards_imported_evidence_and_task_effects() {
    let dir = TempDir::new("traversal-evidence");
    dir.write(
        "operations.aivi",
        r#"
type Traversable F => F Int -> Option (F Int)
func advance = values => traverse (n => Some (n + 1)) values
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func traverseWith = f values => traverse f values
export advance
export traverseWith
"#,
    );
    let path = dir.write("main.aivi", r#"
use operations (advance, traverseWith)
use aivi.core.either (Either, Left, Right)
use aivi.stdio (stdoutWrite)
type Int -> Either Text Int
func incrementRight = n => Right (n + 1)
type Unit -> Int
func firstValue = unit => 3
type Unit -> Int
func secondValue = unit => 4
type Int -> Task Text Int
func emitIncrement = n => n
 ||> 2 -> map firstValue (stdoutWrite "first|")
 ||> _ -> map secondValue (stdoutWrite "second|")
type List Int -> Int
func total = values => reduce (sum n => sum + n) 0 values
value main : Task Text Int = map total (traverseWith emitIncrement [2, 3])
value emptyRight : Either Text (List Int) = Right []
@test
value genericOption : Task Text Bool = pure (advance (Some 2) == Some (Some 3))
@test
value genericEmpty : Task Text Bool = pure (advance [] == Some [])
@test
value importedApplicative : Task Text Bool = pure (traverseWith incrementRight [2, 3] == Right [3, 4])
@test
value authoredEmpty : Task Text Bool = pure (traverseWith incrementRight [] == emptyRight)
@test
value effects : Task Text Bool = map (n => n == 7) main
"#);
    for _ in 0..2 {
        for (command, expected) in [
            ("execute", "first|second|7\n"),
            ("test", "test result: ok. 5 passed; 0 failed; 5 total"),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
                .arg(command)
                .arg(&path)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{command}: {stdout}\n{stderr}");
            assert!(stdout.contains(expected), "{command}: {stdout}");
            assert_eq!(stdout.matches("first|").count(), 1, "{command}: {stdout}");
            assert_eq!(stdout.matches("second|").count(), 1, "{command}: {stdout}");
            assert!(stdout.contains("first|second|"), "{command}: {stdout}");
        }
    }
}

#[test]
fn headless_commands_execute_effectful_task_composition() {
    let dir = TempDir::new("task-composition");
    let path = dir.write(
        "main.aivi",
        r#"
use aivi.stdio (stdoutWrite)
type Unit -> Int
func answer = unit => 42
type Unit -> Task Text Int
func next = unit => map answer (stdoutWrite "second|")
type Unit -> Task Text Int
func sequence = unit => chain next (stdoutWrite "first|")
type Int -> Int
func increment = value => value + 1
type Unit -> (Int -> Int)
func getIncrement = unit => increment
type Int -> Bool
func isAnswer = value => value == 43
value functionTask : Task Text (Int -> Int) = map getIncrement (stdoutWrite "function|")
value nestedTask : Task Text (Task Text Int) = map sequence (stdoutWrite "outer|")
value joinedTask : Task Text Int = join nestedTask
value main : Task Text Int = apply functionTask joinedTask
@test
value mapped : Task Text Bool = map isAnswer main
"#,
    );
    for (command, expected) in [
        ("execute", "function|outer|first|second|43\n"),
        ("test", "test result: ok. 1 passed; 0 failed; 1 total"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg(command)
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{command}: {stdout}\n{stderr}");
        assert!(stdout.contains(expected), "{command}: {stdout}");
        for effect in ["function|", "outer|", "first|", "second|"] {
            assert_eq!(stdout.matches(effect).count(), 1, "{command}: {stdout}");
        }
        assert!(
            stdout.contains("function|outer|first|second|"),
            "{command}: {stdout}"
        );
    }
}

#[test]
fn headless_commands_compose_database_tasks() {
    let dir = TempDir::new("db-task-composition");
    let database = dir.path.join("app.sqlite");
    let path = dir.write(
        "main.aivi",
        &format!(
            r#"
use aivi.db (statement)
use aivi.list (length)
type DatabaseHandle = {{ database: Text }}
value conn = {{ database: "{}" }}
@source db conn
signal database : DatabaseHandle
type List (Map Text Text) -> Int
func rowCount = rows => length rows
type Int -> Int
func increment = value => value + 1
type List (Map Text Text) -> (Int -> Int)
func getIncrement = rows => increment
value query : Task Text (List (Map Text Text)) =
    database.query (statement "select id from users order by id" [])
value functionTask : Task Text (Int -> Int) = map getIncrement query
value nestedTask : Task Text (Task Text Int) = pure (map rowCount query)
value applied : Task Text Int = apply functionTask (join nestedTask)
type Unit -> Task Text Int
func afterCommit = unit => applied
value main : Task Text Int = chain afterCommit (
    database.commit ["users"] [
        statement "create table if not exists users(id integer)" [],
        statement "delete from users" [],
        statement "insert into users values (1), (2)" []
    ]
)
type Int -> Bool
func isThree = value => value == 3
@test
value composed : Task Text Bool = map isThree main
"#,
            database.display()
        ),
    );
    for (command, expected) in [
        ("execute", "3\n"),
        ("test", "test result: ok. 1 passed; 0 failed; 1 total"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg(command)
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{command}: {stdout}\n{stderr}");
        assert!(stdout.contains(expected), "{command}: {stdout}");
    }
}

#[test]
fn headless_database_failures_stop_effects_and_exit_unsuccessfully() {
    let dir = TempDir::new("db-task-failure");
    for (main, success_type) in [
        ("query", "(List (Map Text Text))"),
        ("map rowCount query", "Int"),
        ("chain next query", "Unit"),
    ] {
        let path = dir.write(
            "main.aivi",
            &format!(
                r#"
use aivi.db (statement)
use aivi.list (length)
use aivi.stdio (stdoutWrite)
type DatabaseHandle = {{ database: Text }}
value conn = {{ database: ":memory:" }}
@source db conn
signal database : DatabaseHandle
value query : Task Text (List (Map Text Text)) =
    database.query (statement "select * from missing_table" [])
type List (Map Text Text) -> Int
func rowCount = rows => length rows
type List (Map Text Text) -> Task Text Unit
func next = rows => stdoutWrite "unexpected-effect"
value main : Task Text {success_type} = {main}
"#,
            ),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("execute")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{main}: {stdout}\n{stderr}");
        assert!(
            stderr.contains("no such table"),
            "{main}: {stdout}\n{stderr}"
        );
        assert!(!stdout.contains("unexpected-effect"), "{main}: {stdout}");
    }
}

#[test]
fn test_command_accepts_stdlib_validation_files() {
    for (name, source, summary) in [
        (
            "text",
            concat!(
                "use aivi.text (nonEmpty)\n",
                "@test\n",
                "value text_ok : Task Text Bool = pure (nonEmpty \"Ada\")\n",
            ),
            "test result: ok. 1 passed; 0 failed; 1 total",
        ),
        (
            "list",
            concat!(
                "use aivi.list (length)\n",
                "@test\n",
                "value list_ok : Task Text Bool = pure (length [1, 2, 3] == 3)\n",
            ),
            "test result: ok. 1 passed; 0 failed; 1 total",
        ),
        (
            "option",
            concat!(
                "use aivi.option (getOrElse)\n",
                "@test\n",
                "value option_ok : Task Text Bool = pure (getOrElse 0 (Some 2) == 2)\n",
            ),
            "test result: ok. 1 passed; 0 failed; 1 total",
        ),
        (
            "float",
            concat!(
                "use aivi.core.float (abs)\n",
                "@test\n",
                "value float_ok : Task Text Bool = pure (abs (0.0 - 1.5) == 1.5)\n",
            ),
            "test result: ok. 1 passed; 0 failed; 1 total",
        ),
    ] {
        let dir = TempDir::new(&format!("test-stdlib-{name}"));
        let path = dir.write("main.aivi", source);
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .expect("test command should run");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "expected {name} bundled-stdlib test input to pass `aivi test`, stdout was: {stdout}, stderr was: {stderr}"
        );
        assert!(
            stderr.is_empty(),
            "expected {name} bundled-stdlib test input to keep stderr empty, stderr was: {stderr}"
        );
        assert!(
            stdout.contains(summary),
            "expected success summary for {name} bundled-stdlib test input, stdout was: {stdout}"
        );
    }
}

#[test]
fn test_command_reports_when_workspace_has_no_tests() {
    let dir = TempDir::new("test-no-tests");
    let path = dir.write("main.aivi", "value answer : Int = 42\n");
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(&path)
        .output()
        .expect("test command should run");

    assert!(
        !output.status.success(),
        "expected `aivi test` to fail when no `@test` values exist"
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("no `@test` values found in the loaded workspace"),
        "expected missing-test diagnostic, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_command_accepts_contains_membership_forms() {
    let dir = TempDir::new("test-contains-predicate-forms");
    let path = dir.write(
        "main.aivi",
        concat!(
            "use aivi.list (contains)\n",
            "type Coord = Coord Int Int\n",
            "type Coord -> List Coord -> Bool\n",
            "func member = cell items => contains cell items\n",
            "value items : List Coord = [Coord 0 0, Coord 1 1]\n",
            "value cell : Coord = Coord 1 1\n",
            "value directMatch : Bool = contains cell items\n",
            "value helperMatch : Bool = member cell items\n",
            "value missingMatch : Bool = not (contains (Coord 9 9) items)\n",
            "@test\n",
            "value directContains : Task Text Bool = pure directMatch\n",
            "@test\n",
            "value helperContains : Task Text Bool = pure helperMatch\n",
            "@test\n",
            "value missingContains : Task Text Bool = pure missingMatch\n",
        ),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(&path)
        .output()
        .expect("test command should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected contains membership forms to pass `aivi test`, stdout was: {stdout}, stderr was: {stderr}"
    );
    assert!(
        stderr.is_empty(),
        "expected contains membership forms to keep stderr empty, stderr was: {stderr}"
    );
    assert!(
        stdout.contains("test result: ok. 3 passed; 0 failed; 3 total"),
        "expected success summary for contains membership forms, stdout was: {stdout}"
    );
}

#[test]
fn test_command_selects_one_exact_test_from_the_requested_file() {
    let dir = TempDir::new("test-exact-selection");
    let path = dir.write(
        "main.aivi",
        concat!(
            "@test\n",
            "value selected : Task Text Bool = pure True\n",
            "@test\n",
            "value notSelected : Task Text Bool = pure False\n",
        ),
    );
    let selected = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(&path)
        .arg("selected")
        .output()
        .expect("selected test command should run");

    let stdout = String::from_utf8_lossy(&selected.stdout);
    let stderr = String::from_utf8_lossy(&selected.stderr);
    assert!(
        selected.status.success(),
        "expected exact selected test to pass, stdout was: {stdout}, stderr was: {stderr}"
    );
    assert!(
        stdout.contains("::selected"),
        "selected test was not run: {stdout}"
    );
    assert!(
        !stdout.contains("::notSelected"),
        "unselected test unexpectedly ran: {stdout}"
    );
    assert!(stdout.contains("test result: ok. 1 passed; 0 failed; 1 total"));

    let missing = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(&path)
        .args(["--name", "missing"])
        .output()
        .expect("missing selected test command should run");
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr)
            .contains("no `@test` value named `missing` found in"),
        "missing selection should be actionable, stderr was: {}",
        String::from_utf8_lossy(&missing.stderr)
    );
}

#[test]
fn test_command_runs_stockroom_domain_scenarios() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demos/stockroom/tests.aivi");
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(path)
        .output()
        .expect("stockroom tests should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("test result: ok. 6 passed; 0 failed; 6 total")
    );
}

#[test]
fn headless_tests_execute_imported_method_local_evidence() {
    let dir = TempDir::new("imported-method-evidence");
    dir.write(
        "model/box.aivi",
        r#"
type Box A = Box A
instance Functor Box = { map = f box => box ||> Box a -> Box (f a) }
instance Foldable Box = { reduce = f seed box => box ||> Box a -> f seed a }
instance Traversable Box = { traverse = f box => box ||> Box a -> map Box (f a) }
type Int -> Option Int
func increment = n => Some (n + 1)
type Traversable F => F Int -> Option (F Int)
func advance = box => traverse increment box
type Traversable F => F Int -> Option (F Int)
func forwarded = box => advance box
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func advanceWith = f box => traverse f box
export (Box, advance, forwarded, advanceWith)
"#,
    );
    let path = dir.write(
        "main.aivi",
        r#"
use model.box (Box, advance, forwarded, advanceWith)
type Int -> Option Int
func increment = n => Some (n + 1)
type Int -> List Int
func expand = n => [n, n + 1]
@test
value exercised : Task Text Bool = pure (traverse increment (Box 2) == Some (Box 3))
@test
value generic : Task Text Bool = pure (advance (Box 2) == Some (Box 3))
@test
value nested : Task Text Bool = pure (forwarded (Box 2) == Some (Box 3))
@test
value genericApplicative : Task Text Bool = pure (advanceWith increment (Box 2) == Some (Box 3))
@test
value genericList : Task Text Bool = pure (advanceWith expand (Box 2) == [Box 2, Box 3])
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(path)
        .output()
        .expect("imported evidence test should run");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn headless_tests_pass_lowered_predicate_results_to_class_pure() {
    let dir = TempDir::new("class-pure-predicate-result");
    let path = dir.write(
        "main.aivi",
        r#"
type Either L R = | Left L | Right R
type (L1 -> L2) -> (R1 -> R2) -> (Either L1 R1) -> (Either L2 R2)
func mapBoth = onLeft onRight either => either
 ||> Left v -> Left (onLeft v)
 ||> Right v -> Right (onRight v)
type Text -> Text
func mark = text => text
type Int -> Int
func double = n => n * 2
@test
value compared : Task Text Bool = pure (mapBoth mark double (Right 2) == Right 4)
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(path)
        .output()
        .expect("predicate evidence test should run");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("test result: ok. 1 passed; 0 failed; 1 total")
    );
}
