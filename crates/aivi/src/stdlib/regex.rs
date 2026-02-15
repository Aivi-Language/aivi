pub const MODULE_NAME: &str = "aivi.regex";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.regex
export Regex, RegexError, Match
export compile, test, match, matches, find, findAll, split, replace, replaceAll

use aivi

RegexError = InvalidPattern Text
Match = { full: Text, groups: List (Option Text), start: Int, end: Int }

compile : Text -> Result RegexError Regex
compile = pattern => regex.compile pattern

test : Regex -> Text -> Bool
test = r value => regex.test r value

match : Regex -> Text -> Option Match
match = r value => regex.match r value

matches : Regex -> Text -> List Match
matches = r value => regex.matches r value

find : Regex -> Text -> Option (Int, Int)
find = r value => regex.find r value

findAll : Regex -> Text -> List (Int, Int)
findAll = r value => regex.findAll r value

split : Regex -> Text -> List Text
split = r value => regex.split r value

replace : Regex -> Text -> Text -> Text
replace = r value replacement => regex.replace r value replacement

replaceAll : Regex -> Text -> Text -> Text
replaceAll = r value replacement => regex.replaceAll r value replacement
"#;
