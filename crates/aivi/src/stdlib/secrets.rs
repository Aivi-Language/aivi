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
put = keyId value => secrets.put keyId value

get : SecretKeyId -> Effect SecretError (Option EncryptedBlob)
get = keyId => secrets.get keyId

delete : SecretKeyId -> Effect SecretError Unit
delete = keyId => secrets.delete keyId

blob : SecretKeyId -> Text -> Bytes -> EncryptedBlob
blob = keyId algorithm ciphertext => secrets.makeBlob keyId algorithm ciphertext

blobKeyId : EncryptedBlob -> SecretKeyId
blobKeyId = value => value.keyId

blobAlgorithm : EncryptedBlob -> Text
blobAlgorithm = value => value.algorithm

blobCiphertext : EncryptedBlob -> Bytes
blobCiphertext = value => value.ciphertext

validateBlob : EncryptedBlob -> Bool
validateBlob = value => secrets.validateBlob value
"#;
