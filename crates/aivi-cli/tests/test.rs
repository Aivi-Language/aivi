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
fn derived_equality_invokes_payload_instances_and_scoped_dictionaries() {
    let dir = TempDir::new("derived-payload-equality");
    let path = dir.write(
        "main.aivi",
        r#"
type Tag = Tag Int | Other Int
instance Eq Tag = { (==) = left right => True }
type Box = Box Tag | Empty
domain Tagged over Tag
    wrap : Tag -> Tagged
value leftResult : Result Text Tag = Ok (Tag 1)
value rightResult : Result Text Tag = Ok (Other 2)
value leftValidation : Validation Text Tag = Valid (Tag 1)
value rightValidation : Validation Text Tag = Valid (Other 2)
value invalidValidation : Validation Text Tag = Invalid "missing"
value leftDomain : Tagged = wrap (Tag 1)
value rightDomain : Tagged = wrap (Other 2)
type Eq A => List A -> List A -> Bool
func sameList = left right => left == right
value compareLists : List Tag -> List Tag -> Bool = (==)
@test
value direct : Task Text Bool = pure (Tag 1 == Other 2)
@test
value list : Task Text Bool = pure ([Tag 1] == [Other 2])
@test
value tuple : Task Text Bool = pure ((Tag 1, 1) == (Other 2, 1))
@test
value record : Task Text Bool = pure ({ item: Tag 1 } == { item: Other 2 })
@test
value option : Task Text Bool = pure (Some (Tag 1) == Some (Other 2))
@test
value result : Task Text Bool = pure (leftResult == rightResult)
@test
value validation : Task Text Bool = pure (leftValidation == rightValidation)
@test
value validationTagMatters : Task Text Bool = pure ((leftValidation == invalidValidation) == False)
@test
value domain : Task Text Bool = pure (leftDomain == rightDomain)
@test
value box : Task Text Bool = pure (Box (Tag 1) == Box (Other 2))
@test
value genericList : Task Text Bool = pure (sameList [Tag 1] [Other 2])
@test
value firstClass : Task Text Bool = pure (compareLists [Tag 1] [Other 2])
@test
value lengthMatters : Task Text Bool = pure (([Tag 1] == [Other 2, Tag 3]) == False)
@test
value constructorMatters : Task Text Bool = pure ((Box (Tag 1) == Empty) == False)
@test
value ordinaryPayloadMatters : Task Text Bool = pure (((Tag 1, 1) == (Other 2, 2)) == False)
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
        assert!(stdout.contains("15 passed; 0 failed; 15 total"), "{stdout}");
    }
}

#[test]
fn derived_equality_preserves_imports_reexports_and_method_prerequisites() {
    let dir = TempDir::new("derived-equality-imported-dictionaries");
    dir.write(
        "model.aivi",
        r#"
class Ready A = { ready : A -> Bool }
class Eq A = { (==) : Ready A => A -> A -> Bool }
type Tag = Tag Int | Other Int
type Box A = Box A
value tagLeft : Tag = Tag 1
value boxLeft : Box Tag = Box (Tag 1)
instance Ready Tag = { ready = value => True }
instance Eq Tag = { (==) = left right => ready left }
type (Eq A, Ready A) => List A -> List A -> Bool
func same = left right => left == right
export (Ready, Eq, Tag, Other, Box, same, tagLeft, boxLeft)
"#,
    );
    dir.write(
        "forward.aivi",
        "use model (Ready, Eq as Equality, same)\nexport (Ready, Equality, same)\n",
    );
    let path = dir.write(
        "main.aivi",
        r#"
use forward (Ready, Equality, same)
use model (Tag, Other, Box, tagLeft, boxLeft)
type (Equality A, Ready A) => List A -> List A -> Bool
func nested = left right => same left right
value compare : List Tag -> List Tag -> Bool = (==)
@test
value list : Task Text Bool = pure ([tagLeft] == [Other 2])
@test
value wrapper : Task Text Bool = pure (Box (Some (Tag 1)) == Box (Some (Other 2)))
@test
value crossModuleWrapper : Task Text Bool = pure (boxLeft == Box (Other 2))
@test
value generic : Task Text Bool = pure (nested [Tag 1] [Other 2])
@test
value firstClass : Task Text Bool = pure (compare [Tag 1] [Other 2])
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
fn derived_equality_keeps_distinct_scoped_dictionaries_after_type_substitution() {
    let dir = TempDir::new("derived-equality-distinct-scopes");
    let path = dir.write(
        "main.aivi",
        r#"
class Always A = { (==) : A -> A -> Bool }
class Never A = { (==) : A -> A -> Bool }
instance Always Int = { (==) = left right => True }
instance Never Int = { (==) = left right => False }
type (Always A, Never B) => (List A, List B) -> (List A, List B) -> Bool
func both = left right => left == right
type Always A => List A -> List A -> Bool
func first = left right => left == right
type Never A => List A -> List A -> Bool
func second = left right => left == right
@test
value firstUsesAlways : Task Text Bool = pure (first [1] [2])
@test
value secondUsesNever : Task Text Bool = pure ((second [1] [1]) == False)
@test
value bothKeepTheirSlots : Task Text Bool = pure ((both ([1], [1]) ([2], [1])) == False)
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("3 passed; 0 failed; 3 total"));
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
fn stdlib_traversal_preserves_imported_generic_and_cached_evidence() {
    let dir = TempDir::new("stdlib-traversal-evidence");
    dir.write(
        "operations.aivi",
        r#"
type (Traversable F, Applicative G) => (A -> G B) -> F A -> G (F B)
func traverseWith = transform values => traverse transform values
type (Traversable F, Applicative G) => (A -> G B) -> F A -> G (F B)
func forward = transform values => traverseWith transform values
export forward
"#,
    );
    let path = dir.write("main.aivi", r#"
use operations (forward)
use aivi.core.either (Either, Left, Right)
use aivi.core.dict (Dict, fromList, toList as dictToList)
use aivi.nonEmpty (NonEmptyList, fromHeadTail, toList as nelToList)
use aivi.matrix (Matrix, MatrixError, fromRows, rows)
type Int -> Option Int
func increment = n => Some (n + 1)
type Int -> Either Text Int
func incrementRight = n => Right (n + 1)
value right : Either Text Int = Right 2
type Int -> Option Text
func label = n => Some "item{n}"
value left : Either Text Int = Left "unchanged"
value dictionary : Dict Text Int = fromList [("b", 2), ("a", 1)]
value items : NonEmptyList Int = fromHeadTail 1 [2, 3]
value firstClass : (Int -> Option Int) -> Dict Text Int -> Option (Dict Text Int) = traverse
value expectedDictionary : Either Text (List (Text, Int)) = Right [("a", 2), ("b", 3)]
value traversedDictionary : Either Text (Dict Text Int) = forward incrementRight dictionary
type Matrix Int -> Bool
func checkMatrix = matrix => map rows (forward increment matrix) == Some [[2, 3], [4, 5]]
type Matrix Int -> Bool
func checkMatrixLabels = matrix => map rows (forward label matrix) == Some [["item1", "item2"], ["item3", "item4"]]
value matrixChecked : Bool = fromRows [[1, 2], [3, 4]]
 ||> Ok matrix -> checkMatrix matrix
 ||> Err _ -> False
value matrixLabelsChecked : Bool = fromRows [[1, 2], [3, 4]]
 ||> Ok matrix -> checkMatrixLabels matrix
 ||> Err _ -> False
@test
value eitherGeneric : Task Text Bool = pure (forward increment right == Some (Right 3))
@test
value eitherEmpty : Task Text Bool = pure (forward incrementRight left == Right left)
@test
value dictionaryGeneric : Task Text Bool = pure (map dictToList traversedDictionary == expectedDictionary)
@test
value dictionaryFirstClass : Task Text Bool = pure (map dictToList (firstClass increment dictionary) == Some [("a", 2), ("b", 3)])
@test
value nonEmptyGeneric : Task Text Bool = pure (map nelToList (forward incrementRight items) == Right [2, 3, 4])
@test
value matrixGeneric : Task Text Bool = pure matrixChecked
@test
value eitherChangedPayload : Task Text Bool = pure (forward label right == Some (Right "item2"))
@test
value dictionaryChangedPayload : Task Text Bool = pure (map dictToList (forward label dictionary) == Some [("a", "item1"), ("b", "item2")])
@test
value nonEmptyChangedPayload : Task Text Bool = pure (map nelToList (forward label items) == Some ["item1", "item2", "item3"])
@test
value matrixChangedPayload : Task Text Bool = pure matrixLabelsChecked
"#);
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
fn stdlib_traversal_defers_task_effects_and_stops_after_failure() {
    let cases = [
        (
            "either",
            "Either Text Int",
            "Right 1",
            "value == Right 1",
            "1|",
        ),
        (
            "dict",
            "Dict Text Int",
            "{ entries: [{ key: \"a\", value: 1 }, { key: \"b\", value: 2 }, { key: \"c\", value: 3 }] }",
            "dictToList value == [(\"a\", 1), (\"b\", 2), (\"c\", 3)]",
            "1|2|3|",
        ),
        (
            "nonempty",
            "NonEmptyList Int",
            "fromHeadTail 1 [2, 3]",
            "nelToList value == [1, 2, 3]",
            "1|2|3|",
        ),
        (
            "matrix",
            "Matrix Int",
            "",
            "rows value == [[1, 2], [3, 4]] and width value == 2 and height value == 2",
            "1|2|3|4|",
        ),
    ];
    for (name, carrier, source, check, effects) in cases {
        for failing in [false, true] {
            let dir = TempDir::new("stdlib-traversal-effects");
            dir.write(
                "operations.aivi",
                r#"
type (Traversable F, Applicative G) => (A -> G B) -> F A -> G (F B)
func traverseWith = transform values => traverse transform values
export traverseWith
"#,
            );
            let source_decl = if name == "matrix" {
                "value source : Result MatrixError (Matrix Int) = fromRows [[1, 2], [3, 4]]"
                    .to_owned()
            } else {
                let source = if failing && name == "either" {
                    "Right 2"
                } else {
                    source
                };
                format!("value source : {carrier} = {source}")
            };
            let transform = if failing { "emitFail" } else { "emit" };
            let main = if name == "matrix" {
                format!(
                    "source\n ||> Ok matrix -> map inspect (traverseWith {transform} matrix)\n ||> Err _ -> pure False"
                )
            } else {
                format!("map inspect (traverseWith {transform} source)")
            };
            let program = r#"
use operations (traverseWith)
use aivi.core.either (Either, Left, Right)
use aivi.core.dict (Dict, toList as dictToList)
use aivi.nonEmpty (NonEmptyList, fromHeadTail, toList as nelToList)
use aivi.matrix (Matrix, MatrixError, fromRows, rows, width, height)
use aivi.stdio (stdoutWrite)
use aivi.db (statement)
type DatabaseHandle = { database: Text }
value conn = { database: ":memory:" }
@source db conn
signal database : DatabaseHandle
value query : Task Text (List (Map Text Text)) = database.query (statement "select * from missing_table" [])
type Int -> Unit -> Int
func afterEmit = n unit => n
type Int -> List (Map Text Text) -> Int
func afterQuery = n resultRows => n
type Int -> Unit -> Task Text Int
func failAfter = n unit => map (afterQuery n) query
type Int -> Task Text Int
func emit = n => map (afterEmit n) (stdoutWrite "{n}|")
type Int -> Task Text Int
func emitFail = n => n == 2
 T|> chain (failAfter n) (stdoutWrite "{n}|")
 F|> emit n
type CARRIER -> Bool
func inspect = value => CHECK
SOURCE
value unused : Task Text (List Int) = traverseWith emit [99]
value main : Task Text Bool = MAIN
@test
value executed : Task Text Bool = main
"#.replace("CARRIER", carrier).replace("CHECK", check).replace("SOURCE", &source_decl).replace("MAIN", &main);
            let path = dir.write(&format!("{name}-{failing}.aivi"), &program);
            // Repeat the same source paths to exercise source images and native caches.
            for _ in 0..2 {
                for command in ["execute", "test"] {
                    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
                        .arg(command)
                        .arg(&path)
                        .output()
                        .unwrap();
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    assert_eq!(
                        output.status.success(),
                        !failing,
                        "{name}/{command}: {stdout}\n{stderr}"
                    );
                    let expected_effects = if failing && name == "either" {
                        "2|"
                    } else if failing {
                        "1|2|"
                    } else {
                        effects
                    };
                    assert!(
                        stdout.starts_with(expected_effects),
                        "{name}/{command}: {stdout}"
                    );
                    for effect in ["1|", "2|", "3|", "4|", "99|"] {
                        assert_eq!(
                            stdout.matches(effect).count(),
                            usize::from(expected_effects.contains(effect)),
                            "{name}/{command}: {stdout}"
                        );
                    }
                    if failing {
                        assert!(
                            format!("{stdout}\n{stderr}").contains("no such table"),
                            "{name}/{command}: {stdout}\n{stderr}"
                        );
                    } else if command == "execute" {
                        assert_eq!(stdout, format!("{effects}True\n"));
                    } else {
                        assert!(
                            stdout.contains("1 passed; 0 failed; 1 total"),
                            "{name}/{command}: {stdout}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn constructor_callbacks_execute_import_aliases_and_curried_contracts() {
    let dir = TempDir::new("constructor-callbacks");
    dir.write(
        "maybe.aivi",
        r#"
type Maybe A = Missing | Found A
instance Functor Maybe = {
    map = f maybe => maybe
     ||> Missing -> Missing
     ||> Found a -> Found (f a)
}
instance Apply Maybe = {
    apply = functions values => functions
     ||> Missing -> Missing
     ||> Found f -> map f values
}
instance Applicative Maybe = { pure = a => Found a }
value defaultValue : Maybe Int = Found 7
value factory : Int -> Maybe Int = Found
export Maybe
export Missing
export Found
export defaultValue
export factory
"#,
    );
    dir.write(
        "aliases.aivi",
        "use maybe (Missing as Absent, Found as Present)\nexport Absent\nexport Present\n",
    );
    dir.write("operations.aivi", "type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)\nfunc visit = transform values => traverse transform values\nexport visit\n");
    let path = dir.write("main.aivi", r#"
use operations (visit)
use aliases (Absent, Present)
use maybe (Maybe, Missing, Found, Found as Wrap, Missing as Empty, defaultValue, factory)
value wrappedOptions : List (Option Int) = map Some [1, 2]
value wrappedResults : List (Result Text Int) = map Ok [1, 2]
value wrappedErrors : List (Result Text Int) = map Err ["missing"]
value wrappedValid : List (Validation Text Int) = map Valid [1, 2]
value wrappedInvalid : List (Validation Text Int) = map Invalid ["missing"]
value constructorCallback : Int -> Option Int = Some
value partialConstructorCallback : List Int -> List (Option Int) = map Some
value pipeConstructorCallback : List (Option Int) = [1, 2] |> map Some
type Functor F => (A -> B) -> F A -> F B
func mapped = transform values => map transform values
value sameModulePipe : List (Option Int) = [1, 2] |> mapped Some
value importedPipe : Option (List Int) = [1, 2] |> visit Some
@test
value directSome : Task Text Bool = pure (wrappedOptions == [Some 1, Some 2])
@test
value directOk : Task Text Bool = pure (wrappedResults == [Ok 1, Ok 2])
@test
value directErr : Task Text Bool = pure (wrappedErrors == [Err "missing"])
@test
value directValid : Task Text Bool = pure (wrappedValid == [Valid 1, Valid 2])
@test
value directInvalid : Task Text Bool = pure (wrappedInvalid == [Invalid "missing"])
@test
value firstClassConstructor : Task Text Bool = pure (map constructorCallback [1] == [Some 1])
@test
value partialConstructor : Task Text Bool = pure (partialConstructorCallback [1, 2] == wrappedOptions)
@test
value pipedConstructor : Task Text Bool = pure (pipeConstructorCallback == wrappedOptions)
@test
value pipedSameModuleConstructor : Task Text Bool = pure (sameModulePipe == wrappedOptions)
@test
value pipedImportedConstructor : Task Text Bool = pure (importedPipe == Some [1, 2])
@test
value directAuthored : Task Text Bool = pure (map Found [1, 2] == [Found 1, Found 2])
@test
value aliasAuthored : Task Text Bool = pure (map Wrap [1] == [Found 1])
@test
value reexportAuthored : Task Text Bool = pure (map Present [1] == [Found 1])
@test
value authoredEmptyCallback : Task Text Bool = pure (visit (n => Missing) [1] == Missing)
@test
value aliasEmptyCallback : Task Text Bool = pure (visit (n => Empty) [1] == Missing)
@test
value reexportEmptyCallback : Task Text Bool = pure (visit (n => Absent) [1] == Missing)
@test
value importedOrdinaryValue : Task Text Bool = pure (defaultValue == Found 7)
@test
value importedOrdinaryFunctionValue : Task Text Bool = pure (factory 8 == Found 8)
"#);
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(&path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("18 passed; 0 failed; 18 total"), "{stdout}");
    }
}

#[test]
fn contextual_traversal_callbacks_preserve_imports_captures_and_cached_execution() {
    let dir = TempDir::new("contextual-traversal-callbacks");
    dir.write(
        "maybe.aivi",
        r#"
type Maybe A = Missing | Found A
instance Functor Maybe = {
    map = f maybe => maybe
     ||> Missing -> Missing
     ||> Found a -> Found (f a)
}
instance Apply Maybe = {
    apply = functions values => functions
     ||> Missing -> Missing
     ||> Found f -> map f values
}
instance Applicative Maybe = { pure = a => Found a }
type A -> Maybe A -> A
func unwrapDefault = fallback maybe => maybe
 ||> Missing -> fallback
 ||> Found a -> a
type Maybe A -> Result Text (Maybe A)
func liftResult = maybe => maybe
 ||> Missing -> Ok Missing
 ||> Found a -> Ok (Found a)
export Maybe
export Missing
export Found
export unwrapDefault
export liftResult
"#,
    );
    dir.write(
        "operations.aivi",
        r#"
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func visit = transform values => traverse transform values
export visit
"#,
    );
    dir.write(
        "identity.aivi",
        r#"
type Identity A = Identity A
instance Functor Identity = { map = f identity => identity ||> Identity a -> Identity (f a) }
instance Apply Identity = { apply = functions values => functions ||> Identity f -> map f values }
instance Applicative Identity = { pure = a => Identity a }
type Identity A -> A
func unwrap = identity => identity ||> Identity a -> a
export Identity
export unwrap
"#,
    );
    let path = dir.write(
        "main.aivi",
        r#"
use operations (visit)
use identity (Identity, unwrap)
use maybe (Maybe, Missing, Found, unwrapDefault, liftResult)
use aivi.core.either (Either, Right)
use aivi.core.dict (Dict, fromList)
use aivi.nonEmpty (NonEmptyList, fromHeadTail)
use aivi.matrix (Matrix, MatrixError, fromRows, init as matrixInit, width)
type (Traversable F, Eq (F Int)) => F Int -> Bool
func identityLaw = values => unwrap (traverse Identity values) == values
type Traversable F => A -> F A -> Option (F A)
func fill = captured values => traverse (n => Some captured) values
type Traversable F => A -> F A -> Result Text (F A)
func fillResult = captured values => traverse (n => Ok captured) values
type Traversable F => A -> F A -> Validation Text (F A)
func fillValidation = captured values => traverse (n => Valid captured) values
value firstClass : (Int -> Option Int) -> List Int -> Option (List Int) = visit
value partial : List Int -> Option (List Int) = visit (n => None)
value piped = [1, 2]
 |> visit (n => None)
value nested : List (Option (List Int)) = map (n => visit (m => None) [n]) [1]
value widths : Result MatrixError Int = map width (matrixInit 0 3 (x y => 1))
value right : Either Text Int = Right 2
value dictionary : Dict Text Int = fromList [("a", 1), ("b", 2)]
value emptyDict : Dict Text Int = { entries: [] }
value items : NonEmptyList Int = fromHeadTail 1 [2]
value emptyMaybe : Maybe Int = Missing
value pureMaybe : Maybe Int = pure 5
type Int -> Int
func increment = n => n + 1
type Matrix Int -> Bool
func matrixLaw = matrix => identityLaw matrix
value matrixPassed : Bool = map matrixLaw (fromRows [[1, 2], [3, 4]])
 ||> Ok passed -> passed
 ||> Err _ -> False
@test
value emptyOption : Task Text Bool = pure (visit (n => None) [1, 2] == None)
@test
value emptyChoices : Task Text Bool = pure (visit (n => []) [1, 2] == [])
@test
value mappedOption : Task Text Bool = pure (visit (n => Some (n + 1)) [1, 2] == Some [2, 3])
@test
value firstClassCallback : Task Text Bool = pure (firstClass (n => None) [1] == None)
@test
value partialCallback : Task Text Bool = pure (partial [1] == None)
@test
value pipeCallback : Task Text Bool = pure (piped == None)
@test
value nestedCallback : Task Text Bool = pure (nested == [None])
@test
value capturedCallback : Task Text Bool = pure (fill 9 [1, 2] == Some [9, 9])
@test
value capturedResult : Task Text Bool = pure (fillResult 9 [1, 2] == Ok [9, 9])
@test
value capturedValidation : Task Text Bool = pure (fillValidation 9 [1, 2] == Valid [9, 9])
@test
value listIdentity : Task Text Bool = pure (identityLaw [1, 2])
@test
value eitherIdentity : Task Text Bool = pure (identityLaw right)
@test
value dictIdentity : Task Text Bool = pure (identityLaw dictionary)
@test
value nonEmptyIdentity : Task Text Bool = pure (identityLaw items)
@test
value matrixIdentity : Task Text Bool = pure matrixPassed
@test
value matrixCallback : Task Text Bool = pure (widths == Ok 0)
@test
value emptyDictionary : Task Text Bool = pure (visit (n => None) emptyDict == Some emptyDict)
@test
value authoredMap : Task Text Bool = pure (unwrapDefault 0 (map (n => n + 1) (Found 1)) == 2)
@test
value authoredEmpty : Task Text Bool = pure (unwrapDefault 9 (map (n => n + 1) emptyMaybe) == 9)
@test
value authoredApply : Task Text Bool = pure (unwrapDefault 0 (apply (Found increment) (Found 2)) == 3)
@test
value authoredPure : Task Text Bool = pure (unwrapDefault 0 pureMaybe == 5)
@test
value authoredPartialResult : Task Text Bool = pure (map (unwrapDefault 9) (liftResult emptyMaybe) == Ok 9)
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
        assert!(stdout.contains("22 passed; 0 failed; 22 total"), "{stdout}");
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

#[test]
fn nonempty_comonad_instances_execute_generic_reexported_and_cached_evidence() {
    let dir = TempDir::new("nonempty-comonad");
    dir.write("facade.aivi", "use aivi.nonEmpty (NonEmptyList as NEL, fromHeadTail, toList, length)\nexport (NEL, fromHeadTail, toList, length)\n");
    dir.write(
        "operations.aivi",
        r#"
type Comonad W => W A -> A
func read = values => extract values
type Extend W => (W A -> B) -> W A -> W B
func contexts = observe values => extend observe values
type Comonad W => W A -> W A
func preserve = values => extend extract values
type Comonad W => (W A -> B) -> W A -> B
func observeFirst = observe values => extract (extend observe values)
type Extend W => (W B -> C) -> (W A -> B) -> W A -> C
func observeContext = outer inner values => outer (extend inner values)
type Extend W => (W A -> B) -> (W B -> C) -> W A -> W C
func regroup = inner outer values => extend (observeContext outer inner) values
type Comonad W => W A -> W B -> (A, B)
func readBoth = left right => (extract left, extract right)
export (read, contexts, preserve, observeFirst, regroup, readBoth)
"#,
    );
    let path = dir.write("main.aivi", r#"
use facade (NEL, fromHeadTail, toList, length)
use operations (read, contexts, preserve, observeFirst, regroup, readBoth)
type Int -> Int -> Int
func add = total item => total + item
type NEL Int -> Int
func sum = values => reduce add 0 values
value items : NEL Int = fromHeadTail 1 [2, 3]
value single : NEL Int = fromHeadTail 7 []
value firstClass : (NEL Int -> Int) -> NEL Int -> NEL Int = extend
value partial : NEL Int -> NEL Int = contexts length
@test
value suffixes : Task Text Bool = pure (toList (extend toList items) == [[1, 2, 3], [2, 3], [3]])
@test
value genericExtract : Task Text Bool = pure (read items == 1)
@test
value singleton : Task Text Bool = pure (toList (contexts sum single) == [7])
@test
value firstClassMember : Task Text Bool = pure (toList (firstClass length items) == [3, 2, 1])
@test
value partialMember : Task Text Bool = pure (toList (partial items) == [3, 2, 1])
@test
value extractIdentity : Task Text Bool = pure (toList (preserve items) == [1, 2, 3] and toList (preserve single) == [7])
@test
value observationIdentity : Task Text Bool = pure (observeFirst sum items == sum items and observeFirst sum single == sum single)
@test
value associativity : Task Text Bool = pure (toList (contexts sum (contexts sum items)) == toList (regroup sum sum items) and toList (regroup sum sum items) == [14, 8, 3] and toList (contexts sum (contexts sum single)) == toList (regroup sum sum single) and toList (regroup sum sum single) == [7])
@test
value separateMemberUses : Task Text Bool = pure (readBoth (fromHeadTail "first" []) (fromHeadTail True []) == ("first", True))
"#);
    let tail = (1..2048)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = std::fs::read_to_string(&path).unwrap();
    source.push_str(&format!("\nvalue deep : NEL Int = fromHeadTail 0 [{tail}]\n@test\nvalue stackSafe : Task Text Bool = pure (length (extend extract deep) == 2048 and read deep == 0)\n"));
    std::fs::write(&path, source).unwrap();
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
fn binary_class_instances_execute_imported_partial_and_cached_evidence() {
    let dir = TempDir::new("binary-class-evidence");
    dir.write(
        "arrows.aivi",
        r#"
type Arrow A B = Arrow (A -> B)
type Arrow A B -> A -> B
func runArrow = arrow x => arrow
 ||> Arrow f -> f x
type Arrow B C -> Arrow A B -> Arrow A C
func composeArrow = left right => Arrow (x => runArrow left (runArrow right x))
type A -> A
func identity = x => x
instance Semigroupoid Arrow = { compose = composeArrow }
instance Category Arrow = { id = Arrow identity }
type (A2 -> A1) -> (B1 -> B2) -> Arrow A1 B1 -> Arrow A2 B2
func dimapArrow = before after arrow => Arrow (x => after (runArrow arrow (before x)))
instance Profunctor Arrow = { dimap = dimapArrow }
type Int -> Int
func increment = n => n + 1
type Int -> Int
func twice = n => n * 2
type Text -> Int
func textCount = text => 3
type Int -> Bool
func positive = n => n > 0
value incrementArrow : Arrow Int Int = Arrow increment
value twiceArrow : Arrow Int Int = Arrow twice
value textArrow : Arrow Text Int = Arrow textCount
value boolArrow : Arrow Int Bool = Arrow positive
export (Arrow, runArrow, increment, incrementArrow, twiceArrow, textArrow, boolArrow)
"#,
    );
    dir.write(
        "facade.aivi",
        "use arrows (Arrow as Morphism, runArrow, increment, incrementArrow, twiceArrow, textArrow, boolArrow)\nexport (Morphism, runArrow, increment, incrementArrow, twiceArrow, textArrow, boolArrow)\n",
    );
    let path = dir.write(
        "main.aivi",
        r#"
use facade (Morphism, runArrow, increment, incrementArrow, twiceArrow, textArrow, boolArrow)
use aivi.core.fn (compose as composeFunctions)
type Semigroupoid P => P B C -> P A B -> P A C
func combine = left right => compose left right
type Category P => P B C -> P A B -> P A C
func inheritedCombine = left right => compose left right
type Category P => P A B -> P A B
func keepLeft = arrow => compose id arrow
type Category P => P A B -> P A B
func keepRight = arrow => compose arrow id
type Category P => P A B -> P C D -> (P A B, P C D)
func keepBoth = first second => (compose id first, compose second id)
type Morphism Text Int -> Morphism Int Bool -> Bool
func checkBoth = first second => keepBoth first second
 ||> (textIdentity, boolIdentity) -> runArrow textIdentity "three" == 3 and runArrow boolIdentity 3
value firstClass : Morphism Int Int -> Morphism Int Int -> Morphism Int Int = compose
value partial : Morphism Int Int -> Morphism Int Int = compose incrementArrow
value identityArrow : Morphism Int Int = id
value mapped : Morphism Int Int = dimap increment increment twiceArrow
value differentTypes : Morphism Text Bool = combine boolArrow textArrow
@test
value direct : Task Text Bool = pure (runArrow (compose incrementArrow twiceArrow) 3 == 7)
@test
value generic : Task Text Bool = pure (runArrow (combine incrementArrow twiceArrow) 3 == 7)
@test
value firstClassMethod : Task Text Bool = pure (runArrow (firstClass incrementArrow twiceArrow) 3 == 7)
@test
value partialMethod : Task Text Bool = pure (runArrow (partial twiceArrow) 3 == 7)
@test
value superclass : Task Text Bool = pure (runArrow (inheritedCombine incrementArrow twiceArrow) 3 == 7)
@test
value independentQuantifiers : Task Text Bool = pure (runArrow differentTypes "three")
@test
value leftIdentity : Task Text Bool = pure (runArrow (compose identityArrow incrementArrow) 3 == 4)
@test
value rightIdentity : Task Text Bool = pure (runArrow (compose incrementArrow identityArrow) 3 == 4)
@test
value associativity : Task Text Bool = pure (runArrow (compose incrementArrow (compose twiceArrow incrementArrow)) 3 == runArrow (compose (compose incrementArrow twiceArrow) incrementArrow) 3)
@test
value profunctor : Task Text Bool = pure (runArrow mapped 3 == 9)
@test
value plainFunctionHelper : Task Text Bool = pure (composeFunctions increment increment 3 == 5)
@test
value genericLeftIdentity : Task Text Bool = pure (runArrow (keepLeft incrementArrow) 3 == 4)
@test
value genericRightIdentity : Task Text Bool = pure (runArrow (keepRight incrementArrow) 3 == 4)
@test
value independentIdentityInputs : Task Text Bool = pure (runArrow (keepRight textArrow) "three" == 3)
@test
value independentIdentityOutputs : Task Text Bool = pure (runArrow (keepLeft boolArrow) 3)
@test
value independentValueUses : Task Text Bool = pure (checkBoth textArrow boolArrow)
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
        assert!(stdout.contains("16 passed; 0 failed; 16 total"), "{stdout}");
    }
}

#[test]
fn constructor_exports_execute_generic_partial_nullary_and_cached_uses() {
    let dir = TempDir::new("constructor-export-runtime");
    dir.write(
        "models.aivi",
        r#"
type Box A = Box A
type Pair A B = Pair A B
type Flag = | Flag
type (A -> B) -> Box A -> Box B
func mapBox = f box => box ||> Box x -> Box (f x)
instance Functor Box = { map = mapBox }
export (Box, Pair, Flag)
"#,
    );
    dir.write("facade.aivi", "use models (Box as Container, Pair as Product, Flag as Mark)\nexport (Container, Product, Mark)\n");
    dir.write("bridge.aivi", "use facade (Container as Wrapped, Product as Together, Mark as Token)\nexport (Wrapped, Together, Token)\n");
    let path = dir.write(
        "main.aivi",
        r#"
use bridge (Wrapped, Together, Token)
type Functor F => (A -> B) -> F A -> F B
func transform = f values => map f values
type Int -> Int
func increment = n => n + 1
type Wrapped A -> A
func read = value => value ||> Wrapped x -> x
type Together A B -> B
func second = pair => pair ||> Together first last -> last
value wrap : Text -> Wrapped Text = Wrapped
value pair : Text -> Together Int Text = Together 7
type Token -> Bool
func isToken = value => value ||> Token -> True
value token : Token = Token
@test
value aliasPattern : Task Text Bool = pure (read (Wrapped 3) == 3)
@test
value firstClass : Task Text Bool = pure (read (wrap "kept") == "kept")
@test
value partial : Task Text Bool = pure (second (pair "second") == "second")
@test
value nullary : Task Text Bool = pure (isToken token)
@test
value publicInstance : Task Text Bool = pure (read (map increment (Wrapped 3)) == 4)
@test
value genericInstance : Task Text Bool = pure (read (transform increment (Wrapped 3)) == 4)
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
