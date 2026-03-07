pub const MODULE_NAME: &str = "aivi.ui.forms";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.forms
export Field
export field, setValue, touch
export validate, errors, visibleErrors
export allOf, rule, required, minLength, maxLength, email

use aivi
use aivi.regex
use aivi.collections
use aivi.text
use aivi.validation

Field A = {
  value: A
  touched: Bool
  dirty: Bool
}

field : A -> Field A
field = value => {
  value: value
  touched: False
  dirty: False
}

setValue : A -> Field A -> Field A
setValue = value state =>
  state <| {
    value: value
    dirty: True
  }

touch : Field A -> Field A
touch = state =>
  state <| {
    touched: True
  }

validate : (A -> Validation (List E) B) -> Field A -> Validation (List E) B
validate = validator state => validator state.value

errorsFrom : Validation (List E) A -> List E
errorsFrom = validationResult =>
  fold (errs => errs) (_ => []) validationResult

errors : (A -> Validation (List E) B) -> Field A -> List E
errors = validator state => errorsFrom (validate validator state)

visibleErrors : Bool -> (A -> Validation (List E) B) -> Field A -> List E
visibleErrors = submitted validator state =>
  if submitted || state.touched
    then errors validator state
    else []

collectErrors : List (A -> Validation (List E) B) -> A -> List E
collectErrors = validators value => validators match
  | []                 => []
  | [validator, ...rest] => errorsFrom (validator value) ++ collectErrors rest value

allOf : List (A -> Validation (List E) B) -> A -> Validation (List E) A
allOf = validators value => {
  errs = collectErrors validators value
  if List.isEmpty errs
    then Valid value
    else Invalid errs
}

rule : Text -> (A -> Bool) -> A -> Validation (List Text) A
rule = message predicate value =>
  if predicate value
    then Valid value
    else Invalid [message]

required : Text -> Validation (List Text) Text
required = value => {
  trimmed = text.trim value
  if text.isEmpty trimmed
    then Invalid ["This field is required"]
    else Valid value
}

minLength : Int -> Text -> Validation (List Text) Text
minLength = count value => {
  trimmed = text.trim value
  if text.isEmpty trimmed || text.length value >= count
    then Valid value
    else Invalid ["Must be at least {text.toText count} characters"]
}

maxLength : Int -> Text -> Validation (List Text) Text
maxLength = count value => {
  trimmed = text.trim value
  if text.isEmpty trimmed || text.length value <= count
    then Valid value
    else Invalid ["Must be at most {text.toText count} characters"]
}

emailPattern : Regex
emailPattern = ~r/^[^@\s]+@[^@\s]+\.[^@\s]+$/

email : Text -> Validation (List Text) Text
email = value => {
  trimmed = text.trim value

  if text.isEmpty trimmed || regex.test emailPattern trimmed
    then Valid value
    else Invalid ["Enter a valid email address"]
}
"#;
