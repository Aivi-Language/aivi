#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "integration-tests" / "syntax"


def write(rel: str, text: str) -> None:
    path = OUT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    if not text.endswith("\n"):
        text += "\n"
    path.write_text(text, encoding="utf-8")


def main() -> None:
    cases: dict[str, str] = {
        "bindings/basic.aivi": """@no_prelude
module integrationTests.syntax.bindings.basic

use aivi
use aivi.testing (assertEq)

x = 1
y = if x < 2 then 3 else 4
pair = (x, y)

@test
basicBindings = effect {
  _ <- assertEq x 1
  _ <- assertEq y 3
  _ <- assertEq pair (1, 3)
}
""",
        "bindings/recursion.aivi": """@no_prelude
module integrationTests.syntax.bindings.recursion

use aivi
use aivi.testing (assertEq)

sum = xs => xs ?
  | []           => 0
  | [x, ...rest] => x + sum rest

@test
recursionWorks = effect {
  _ <- assertEq (sum [1, 2, 3]) 6
}
""",
        "functions/multi_arg_and_sig.aivi": """@no_prelude
module integrationTests.syntax.functions.multiArgAndSig

use aivi
use aivi.testing (assertEq)

add : Int -> Int -> Int
add = a b => a + b

applyTwice : (A -> A) -> A -> A
applyTwice = f x => f (f x)

inc = n => n + 1

@test
functionsWork = effect {
  _ <- assertEq (add 2 3) 5
  _ <- assertEq (applyTwice inc 1) 3
}
""",
        "types/unions_and_aliases.aivi": """@no_prelude
module integrationTests.syntax.types.unionsAndAliases

use aivi
use aivi.testing (assertEq)

Color = Red | Green | Blue
Pair A B = (A, B)

mk : A -> B -> Pair A B
mk = a b => (a, b)

toInt = c => c ?
  | Red   => 1
  | Green => 2
  | Blue  => 3

@test
typesWork = effect {
  _ <- assertEq (mk 1 "x") (1, "x")
  _ <- assertEq (toInt Red) 1
}
""",
        "predicates/implicit_and_explicit.aivi": """@no_prelude
module integrationTests.syntax.predicates.implicitAndExplicit

use aivi
use aivi.testing (assertEq)

filter = pred xs => xs ?
  | []           => []
  | [x, ...rest] =>
if pred x then[x, ...(filter pred rest)] else filter pred rest

values = [1, 2, 3, 4, 5]
gt2 = values |> filter (_ > 2)
expected = [3, 4, 5]

@test
predicatesWork = effect {
  _ <- assertEq gt2 expected
}
""",
        "patching/record_patch_basic.aivi": """@no_prelude
module integrationTests.syntax.patching.recordPatchBasic

use aivi
use aivi.testing (assertEq)

user = { name: "A", age: 30 }
older = user <| { age: _ + 1 }

@test
patchingWorks = effect {
  _ <- assertEq older.age 31
}
""",
        "domains/import_and_suffix_literals.aivi": """@no_prelude
module integrationTests.syntax.domains.importAndSuffixLiterals

use aivi
use aivi.duration
use aivi.duration (domain Duration)
use aivi.testing (assertEq)

span0 : Span
span0 = { millis: 0 }
span1 = span0 + 30s
span2 = span0 + 1min

@test
suffixLiteralsWork = effect {
  _ <- assertEq span1.millis 30000
  _ <- assertEq span2.millis 60000
}
""",
        "domains/suffix_application_expr.aivi": """@no_prelude
module integrationTests.syntax.domains.suffixApplicationExpr

use aivi
use aivi.duration
use aivi.duration (domain Duration)
use aivi.testing (assertEq)

span0 : Span
span0 = { millis: 0 }
x = 2
delta : Delta
delta = (x)min
span1 = span0 + delta

@test
suffixApplicationWorks = effect {
  _ <- assertEq span1.millis 120000
}
""",
        "pattern_matching/lists_and_records.aivi": """@no_prelude
module integrationTests.syntax.patternMatching.listsAndRecords

use aivi
use aivi.testing (assertEq)

head = xs => xs ?
  | []       => None
  | [x, ...rest] => Some x

getName = user => user ?
  | { name } => Some name
  | _        => None

@test
patternsWork = effect {
  _ <- assertEq (head([1, 2])) (Some 1)
  _ <- assertEq (head([])) None
  _ <- assertEq (getName { name: "A", age: 1 }) (Some "A")
}
""",
        "pattern_matching/guards_when.aivi": """@no_prelude
module integrationTests.syntax.patternMatching.guardsWhen

use aivi
use aivi.testing (assertEq)

classify =
  | n when n < 0   => "negative"
  | 0              => "zero"
  | n when n < 10  => "small"
  | n when n < 100 => "medium"
  | _              => "large"

@test
guardsWork = effect {
  _ <- assertEq (classify (-1)) "negative"
  _ <- assertEq (classify 0) "zero"
  _ <- assertEq (classify 42) "medium"
}
""",
        "effects/attempt_and_match.aivi": """@no_prelude
module integrationTests.syntax.effects.attemptAndMatch

use aivi
use aivi.testing (assertEq)

@test
attemptWorks = effect {
  res <- attempt (fail "nope")
  res ?
    | Ok _  => fail "unexpected"
    | Err e => assertEq e "nope"
}
""",
        "effects/or_sugar.aivi": """@no_prelude
module integrationTests.syntax.effects.orSugar

use aivi
use aivi.testing (assertEq)

@test
orSugarWorks = effect {
  txt <- load (env.get "AIVI__MISSING_TEST__") or "fallback"
  _ <- assertEq txt "fallback"
}
""",
        "modules/use_alias_and_selective_imports.aivi": """@no_prelude
module integrationTests.syntax.modules.useAliasAndSelectiveImports

use aivi.text as text
use aivi.text (length, toUpper)
use aivi.testing (assertEq)

len = length
upper = toUpper
qualifiedLen = text.length

@test
importsWork = effect {
  _ <- assertEq (len "abc") 3
  _ <- assertEq (upper "ab") "AB"
  _ <- assertEq (qualifiedLen "abcd") 4
}
""",
        "operators/precedence_and_pipes.aivi": """@no_prelude
module integrationTests.syntax.operators.precedenceAndPipes

use aivi
use aivi.testing (assertEq)

map = f xs => xs ?
  | []           => []
  | [x, ...rest] => [f x, ...(map f rest)]

values = [1, 2, 3]
out = values |> map (n => n * 2 + 1)
expected = [3, 5, 7]

@test
operatorsWork = effect {
  _ <- assertEq out expected
}
""",
        "sigils/basic.aivi": """@no_prelude
module integrationTests.syntax.sigils.basic

use aivi
use aivi.regex
use aivi.testing (assert, assertEq)
use aivi.url (domain Url)

pattern = ~r/\\w+@\\w+\\.\\w+/
endpoint = ~u(https://api.example.com)

@test
sigilsWork = effect {
  _ <- assert (regex.test pattern "a@b.com")
  _ <- assertEq endpoint.protocol "https"
}
""",
        "sigils/collections_structured.aivi": """@no_prelude
module integrationTests.syntax.sigils.collectionsStructured

use aivi (Map, Set)
use aivi.collections (domain Collections)
use aivi.testing (assert, assertEq)

users = ~map{
  "id-1" => { name: "Alice" }
  "id-2" => { name: "Bob" }
}

baseTags = ~set["base"]
tags = ~set[...baseTags, "hot", "new"]

@test
structuredSigilsWork = effect {
  _ <- assertEq (Map.size users) 2
  _ <- assert (Set.has "hot" tags)
}
""",
        "decorators/static_and_test.aivi": """@no_prelude
module integrationTests.syntax.decorators.staticAndTest

use aivi
use aivi.testing (assertEq)

@static
embedded = "ok"

@test
staticIsAvailable = effect {
  _ <- assertEq embedded "ok"
}
""",
        "resources/basic_resource_block.aivi": """@no_prelude
module integrationTests.syntax.resources.basicResourceBlock

use aivi
use aivi.testing (assert)

managedFile : Text -> Resource Text FileHandle
managedFile = path => resource {
  handle <- file.open path
  yield handle
  _ <- file.close handle
}

@test
resourcesTypecheck = effect {
  _ <- assert True
}
""",
        "external_sources/env_get_and_default.aivi": """@no_prelude
module integrationTests.syntax.externalSources.envGetAndDefault

use aivi
use aivi.testing (assertEq)

@test
envDefaultWorks = effect {
  port <- load (env.get "AIVI__MISSING_TEST_PORT__") or "3000"
  _ <- assertEq port "3000"
}
""",
        "external_sources/file_read_source_value.aivi": """@no_prelude
module integrationTests.syntax.externalSources.fileReadSourceValue

use aivi
use aivi.text
use aivi.testing (assert)

cfgSource : Source File Text
cfgSource = file.read "integration-tests/legacy/i18n/app.en-US.properties"

@test
fileSourceLoads = effect {
  txt <- load cfgSource
  _ <- assert (text.length txt > 0)
}
""",
        "generators/basic_yield.aivi": """@no_prelude
module integrationTests.syntax.generators.basicYield

use aivi
use aivi.testing (assert)

gen = generate {
  yield 1
  yield 2
  yield 3
}

@test
generatorsExist = effect {
  _ = text.toText gen
  _ <- assert True
}
""",
        "pattern_matching/guarded_case_with_if.aivi": """@no_prelude
module integrationTests.syntax.patternMatching.guardedCaseWithIf

use aivi
use aivi.testing (assertEq)

clamp01 = x =>
if x < 0 then 0 else if x > 1 then 1 else x

@test
ifsWork = effect {
  _ <- assertEq (clamp01 (-1)) 0
  _ <- assertEq (clamp01 0) 0
  _ <- assertEq (clamp01 2) 1
}
""",
        "operators/operator_sections_and_names.aivi": """@no_prelude
module integrationTests.syntax.operators.operatorSectionsAndNames

use aivi
use aivi.duration
use aivi.testing (assertEq)

addDelta : Span -> Delta -> Span
addDelta = (+)

subDelta : Span -> Delta -> Span
subDelta = (-)

@test
operatorSectionsWork = effect {
  span0 = { millis: 0 }
  _ <- assertEq ((addDelta span0 1s).millis) 1000
  _ <- assertEq ((subDelta { millis: 1000 } 1s).millis) 0
}
""",
    }

    for rel, text in cases.items():
        write(rel, text)


if __name__ == "__main__":
    main()
