pub const MODULE_NAME: &str = "aivi.goa";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.goa
export GoaAccount, GoaToken
export listAccounts, getAccessToken
export accountKey, filterByKey

use aivi

GoaAccount = { key: Text }
GoaToken = { token: Text, expiresUnix: Int }

listAccounts : Effect Text (List GoaAccount)
listAccounts = load (goa.listAccounts Unit)

getAccessToken : Text -> Effect Text GoaToken
getAccessToken = key => load (goa.getAccessToken key)

accountKey : GoaAccount -> Text
accountKey = account => account.key

filterByKey : Text -> List GoaAccount -> List GoaAccount
filterByKey = key accounts => accounts match
  | [] => []
  | [x, ...xs] =>
      if x.key == key
      then [x, ...filterByKey key xs]
      else filterByKey key xs
"#;
