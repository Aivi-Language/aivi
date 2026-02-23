pub const MODULE_NAME: &str = "aivi.secrets";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.secrets
export SecretKeyId, EncryptedBlob, SecretError
export put, get, delete
export blob, blobKeyId, blobAlgorithm, blobCiphertext
export validateBlob

use aivi

SecretKeyId = Text
SecretError = Text
EncryptedBlob = {
  keyId: SecretKeyId
  algorithm: Text
  ciphertext: Bytes
}

put : SecretKeyId -> EncryptedBlob -> Effect SecretError Unit
put = keyId value => pure Unit

get : SecretKeyId -> Effect SecretError (Option EncryptedBlob)
get = keyId => pure None

delete : SecretKeyId -> Effect SecretError Unit
delete = keyId => pure Unit

blob : SecretKeyId -> Text -> Bytes -> EncryptedBlob
blob = keyId algorithm ciphertext => { keyId, algorithm, ciphertext }

blobKeyId : EncryptedBlob -> SecretKeyId
blobKeyId = value => value.keyId

blobAlgorithm : EncryptedBlob -> Text
blobAlgorithm = value => value.algorithm

blobCiphertext : EncryptedBlob -> Bytes
blobCiphertext = value => value.ciphertext

validateBlob : EncryptedBlob -> Bool
validateBlob = value =>
  if value.keyId == "" then False else if value.algorithm == "" then False else True
"#;
