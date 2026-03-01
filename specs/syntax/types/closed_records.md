# 3.3 Closed Records

Records are:

* structural
* closed

<<< ../../snippets/from_md/syntax/types/closed_records_01.aivi{aivi}

To create a record value, use a record literal:

<<< ../../snippets/from_md/syntax/types/closed_records_02.aivi{aivi}

Record literals can spread existing records:

<<< ../../snippets/from_md/syntax/types/closed_records_03.aivi{aivi}

Spreads merge fields left-to-right; later entries override earlier ones.

Functions specify an **exact record shape** in type signatures.

<<< ../../snippets/from_md/syntax/types/closed_records_04.aivi{aivi}
