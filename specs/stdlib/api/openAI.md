# OpenAI API Client

<!-- quick-info: {"kind":"module","name":"aivi.api.openai"} -->
A fully type-safe client for the [OpenAI REST API](https://platform.openai.com/docs/api-reference).
Covers Chat Completions, Models, Embeddings, Image Generation, and Audio — all built on top of `aivi.rest`.
<!-- /quick-info -->

<div class="import-badge">use aivi.api.openai</div>

---

## Client

Every function in this module takes a `Client` as its first argument.
Construct one with `client`, then pass it through your program.

```aivi
use aivi.api.openai

apiKey = ApiKey (env "OPENAI_API_KEY")
ai     = openai.client apiKey
```

### `ApiKey`

A branded `Text` wrapper that prevents accidentally passing a raw string where an API key is expected.

```aivi
ApiKey = Text!
```

### `Client`

```aivi
Client = {
  apiKey   : ApiKey
  baseUrl  : Url
  timeoutMs: Int
}
```

| Field | Type | Default |
| --- | --- | --- |
| `apiKey` | `ApiKey` | _(required)_ |
| `baseUrl` | `Url` | `https://api.openai.com/v1` |
| `timeoutMs` | `Int` | `30000` |

### `client`

```aivi
client : ApiKey -> Client
```

Builds a `Client` with default `baseUrl` and `timeoutMs`.

```aivi
client = apiKey =>
  {
    apiKey
    baseUrl: ~u(https://api.openai.com/v1)
    timeoutMs: 30000
  }
```

---

## Shared Types

### `ModelId`

A branded `Text` wrapper for a model identifier such as `"gpt-4o"` or `"text-embedding-3-small"`.

```aivi
ModelId = Text!
```

Common values are available as constants:

```aivi
gpt4o          : ModelId
gpt4oMini      : ModelId
gpt41          : ModelId
o3             : ModelId
o4Mini         : ModelId
textEmbedding3Small : ModelId
textEmbedding3Large : ModelId
dallE3         : ModelId
tts1           : ModelId
whisper1       : ModelId
```

### `ApiError`

Returned in the `Err` branch of every API call.

```aivi
ApiError = {
  code   : Option Text
  message: Text
  param  : Option Text
  kind   : Text
}
```

### `Usage`

Token-usage statistics included in completion and embedding responses.

```aivi
Usage = {
  promptTokens    : Int
  completionTokens: Int
  totalTokens     : Int
}
```

### `Page A`

Cursor-paginated list wrapper used by list endpoints.

```aivi
Page A = {
  object : Text
  data   : List A
  firstId: Option Text
  lastId : Option Text
  hasMore: Bool
}
```

---

## Chat Completions

<!-- quick-info: {"kind":"section","name":"chat completions"} -->
Send a conversation to a model and receive a reply.
<!-- /quick-info -->

### Types

#### `ChatRole`

```aivi
ChatRole = System | User | Assistant | Tool | Developer
```

#### `Message`

A single turn in a conversation. Use `userMsg`, `systemMsg`, or `assistantMsg` helpers instead of
constructing this record directly.

```aivi
Message = {
  role   : ChatRole
  content: Text
  name   : Option Text
}
```

#### `FinishReason`

Why the model stopped generating tokens.

```aivi
FinishReason = Stop | Length | ToolCalls | ContentFilter | FunctionCall
```

#### `ResponseMessage`

The message returned by the model.

```aivi
ResponseMessage = {
  role   : Text
  content: Option Text
  refusal: Option Text
}
```

#### `Choice`

One candidate completion.

```aivi
Choice = {
  index       : Int
  message     : ResponseMessage
  finishReason: FinishReason
}
```

#### `ChatRequest`

Full request options for `/v1/chat/completions`.

```aivi
ChatRequest = {
  model            : ModelId
  messages         : List Message
  maxTokens        : Option Int
  temperature      : Option Float
  topP             : Option Float
  n                : Option Int
  stop             : Option (List Text)
  presencePenalty  : Option Float
  frequencyPenalty : Option Float
  seed             : Option Int
  user             : Option Text
  reasoningEffort  : Option ReasoningEffort
}
```

#### `ReasoningEffort`

Effort level hint for reasoning models (`o3`, `o4-mini`, …).

```aivi
ReasoningEffort = EffortLow | EffortMedium | EffortHigh
```

#### `ChatResponse`

Successful response from `/v1/chat/completions`.

```aivi
ChatResponse = {
  id               : Text
  object           : Text
  created          : Int
  model            : Text
  choices          : List Choice
  usage            : Usage
  systemFingerprint: Option Text
}
```

### Functions

| Function | Signature | Description |
| --- | --- | --- |
| **chat** client request | `Client -> ChatRequest -> Effect ApiError ChatResponse` | Send a chat conversation. |
| **chatSimple** client model messages | `Client -> ModelId -> List Message -> Effect ApiError ChatResponse` | Minimal chat call with defaults. |
| **userMsg** text | `Text -> Message` | Build a `User` message. |
| **systemMsg** text | `Text -> Message` | Build a `System` message. |
| **assistantMsg** text | `Text -> Message` | Build an `Assistant` message. |
| **extractText** response | `ChatResponse -> Option Text` | Pull the text of the first choice. |

### Usage example

```aivi
use aivi.api.openai

ai = openai.client (ApiKey (env "OPENAI_API_KEY"))

answer : Text -> Effect ApiError Text
answer = question => do Effect {
  resp <- openai.chat ai {
    model: openai.gpt4o
    messages:
      [ openai.systemMsg "You are a concise assistant."
      , openai.userMsg question
      ]
    maxTokens: Some 256
    temperature: Some 0.7
    topP: None
    n: None
    stop: None
    presencePenalty: None
    frequencyPenalty: None
    seed: None
    user: None
    reasoningEffort: None
  }
  resp
    |> openai.extractText
    |> Option.withDefault "No response."
    |> pure
}
```

---

## Models

<!-- quick-info: {"kind":"section","name":"models"} -->
List and inspect the models available to your API key.
<!-- /quick-info -->

### Types

#### `Model`

```aivi
Model = {
  id     : Text
  object : Text
  created: Int
  ownedBy: Text
}
```

### Functions

| Function | Signature | Description |
| --- | --- | --- |
| **listModels** client | `Client -> Effect ApiError (Page Model)` | Returns all available models. |
| **getModel** client modelId | `Client -> Text -> Effect ApiError Model` | Retrieves metadata for a single model. |
| **deleteModel** client modelId | `Client -> Text -> Effect ApiError DeleteResult` | Deletes a fine-tuned model. |

#### `DeleteResult`

```aivi
DeleteResult = {
  id     : Text
  object : Text
  deleted: Bool
}
```

### Usage example

```aivi
use aivi.api.openai
use aivi.list (map)

printModels : Client -> Effect ApiError Unit
printModels = ai => do Effect {
  page <- openai.listModels ai
  page.data
    |> map .id
    |> List.forEach (id => console.log id)
}
```

---

## Embeddings

<!-- quick-info: {"kind":"section","name":"embeddings"} -->
Convert text into a dense vector for semantic search, clustering, and classification.
<!-- /quick-info -->

### Types

#### `EmbeddingInput`

```aivi
EmbeddingInput = SingleText Text | MultiText (List Text)
```

#### `EncodingFormat`

```aivi
EncodingFormat = FloatFormat | Base64Format
```

#### `EmbeddingRequest`

```aivi
EmbeddingRequest = {
  model         : ModelId
  input         : EmbeddingInput
  encodingFormat: Option EncodingFormat
  dimensions    : Option Int
  user          : Option Text
}
```

#### `EmbeddingObject`

A single embedding vector in the response.

```aivi
EmbeddingObject = {
  index    : Int
  embedding: List Float
  object   : Text
}
```

#### `EmbeddingUsage`

```aivi
EmbeddingUsage = {
  promptTokens: Int
  totalTokens : Int
}
```

#### `EmbeddingResponse`

```aivi
EmbeddingResponse = {
  object: Text
  model : Text
  data  : List EmbeddingObject
  usage : EmbeddingUsage
}
```

### Functions

| Function | Signature | Description |
| --- | --- | --- |
| **embed** client request | `Client -> EmbeddingRequest -> Effect ApiError EmbeddingResponse` | Create embeddings for the given input. |
| **embedText** client model text | `Client -> ModelId -> Text -> Effect ApiError (List Float)` | Embed a single string, returning the raw vector. |
| **cosineSimilarity** a b | `List Float -> List Float -> Float` | Compute cosine similarity between two embedding vectors. |

### Usage example

```aivi
use aivi.api.openai

ai = openai.client (ApiKey (env "OPENAI_API_KEY"))

similarity : Text -> Text -> Effect ApiError Float
similarity = a b => do Effect {
  va <- openai.embedText ai openai.textEmbedding3Small a
  vb <- openai.embedText ai openai.textEmbedding3Small b
  pure (openai.cosineSimilarity va vb)
}
```

---

## Image Generation

<!-- quick-info: {"kind":"section","name":"images"} -->
Generate, edit, and vary images with DALL·E 2, DALL·E 3, or `gpt-image-1`.
<!-- /quick-info -->

### Types

#### `ImageSize`

```aivi
ImageSize
  = Auto
  | Square256
  | Square512
  | Square1024
  | Landscape1792
  | Portrait1792
  | Landscape1536
  | Portrait1536
```

#### `ImageQuality`

```aivi
ImageQuality = QualityAuto | Standard | Hd | Low | Medium | High
```

#### `ImageStyle`

Only supported by DALL·E 3.

```aivi
ImageStyle = Vivid | Natural
```

#### `ImageResponseFormat`

```aivi
ImageResponseFormat = Url | B64Json
```

#### `ImageRequest`

```aivi
ImageRequest = {
  prompt        : Text
  model         : Option ModelId
  n             : Option Int
  size          : Option ImageSize
  quality       : Option ImageQuality
  style         : Option ImageStyle
  responseFormat: Option ImageResponseFormat
  user          : Option Text
}
```

#### `GeneratedImage`

```aivi
GeneratedImage = {
  url           : Option Text
  b64Json       : Option Text
  revisedPrompt : Option Text
}
```

#### `ImageResponse`

```aivi
ImageResponse = {
  created: Int
  data   : List GeneratedImage
}
```

### Functions

| Function | Signature | Description |
| --- | --- | --- |
| **generateImage** client request | `Client -> ImageRequest -> Effect ApiError ImageResponse` | Generate images from a text prompt. |
| **generateImageSimple** client prompt | `Client -> Text -> Effect ApiError (List GeneratedImage)` | Generate one 1024×1024 DALL·E 3 image. |

### Usage example

```aivi
use aivi.api.openai

ai = openai.client (ApiKey (env "OPENAI_API_KEY"))

generateLogo : Text -> Effect ApiError Text
generateLogo = description => do Effect {
  resp <- openai.generateImageSimple ai description
  resp match
    | [{ url: Some imageUrl }, ..._] => pure imageUrl
    | _                              => Err { code: None, message: "No image returned", param: None, kind: "client" }
}
```

---

## Audio

<!-- quick-info: {"kind":"section","name":"audio"} -->
Convert text to speech or transcribe audio files to text.
<!-- /quick-info -->

### Types

#### `Voice`

Available voices for text-to-speech.

```aivi
Voice = Alloy | Ash | Ballad | Coral | Echo | Fable | Nova | Onyx | Sage | Shimmer
```

#### `AudioFormat`

Output format for speech synthesis.

```aivi
AudioFormat = Mp3 | Opus | Aac | Flac | Wav | Pcm
```

#### `SpeechRequest`

```aivi
SpeechRequest = {
  model          : ModelId
  input          : Text
  voice          : Voice
  responseFormat : Option AudioFormat
  speed          : Option Float
}
```

#### `TranscriptionRequest`

```aivi
TranscriptionRequest = {
  file          : Bytes
  model         : ModelId
  language      : Option Text
  prompt        : Option Text
  temperature   : Option Float
}
```

#### `Transcription`

```aivi
Transcription = { text: Text }
```

### Functions

| Function | Signature | Description |
| --- | --- | --- |
| **createSpeech** client request | `Client -> SpeechRequest -> Effect ApiError Bytes` | Generate spoken audio from text. |
| **transcribe** client request | `Client -> TranscriptionRequest -> Effect ApiError Transcription` | Transcribe audio to text. |

### Usage example

```aivi
use aivi.api.openai
use aivi.file

ai  = openai.client (ApiKey (env "OPENAI_API_KEY"))
tts = openai.createSpeech ai {
  model: openai.tts1
  input: "Hello from AIVI!"
  voice: openai.Nova
  responseFormat: Some openai.Mp3
  speed: None
}

saveAudio : Effect ApiError Unit
saveAudio = do Effect {
  bytes <- tts
  file.write ~p(./output.mp3) bytes
}
```

---

## Request Builder Helpers

These helpers reduce boilerplate for common patterns:

```aivi
// Construct request with all optional fields set to None
minimalChatRequest : ModelId -> List Message -> ChatRequest
minimalChatRequest = model messages =>
  {
    model
    messages
    maxTokens: None
    temperature: None
    topP: None
    n: None
    stop: None
    presencePenalty: None
    frequencyPenalty: None
    seed: None
    user: None
    reasoningEffort: None
  }

// Override temperature on any ChatRequest
withTemperature : Float -> ChatRequest -> ChatRequest
withTemperature = t req => req <| { temperature: Some t }

// Override max tokens on any ChatRequest
withMaxTokens : Int -> ChatRequest -> ChatRequest
withMaxTokens = n req => req <| { maxTokens: Some n }
```

---

## Error Handling

All API functions return `Effect ApiError A`. Use `attempt` to convert to `Result`:

```aivi
use aivi.api.openai

safeChat : Client -> ChatRequest -> Effect Text (Option ChatResponse)
safeChat = ai req => do Effect {
  result <- attempt (openai.chat ai req)
  result match
    | Ok resp            => pure (Some resp)
    | Err { message, _ } => do Effect {
        console.log "OpenAI error: {message}"
        pure None
      }
}
```

---

## Rate Limiting & Retries

The client supports automatic retry with exponential back-off via the underlying `rest` module.
Pass a customised `Client` to enable it:

```aivi
robustClient : ApiKey -> Client
robustClient = apiKey =>
  openai.client apiKey <| { timeoutMs: 60000 }

// Retry is handled at the rest.fetch layer via retryCount in the request.
```
