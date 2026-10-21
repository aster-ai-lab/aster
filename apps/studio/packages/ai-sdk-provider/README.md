# @cherrydesktop/ai-sdk-provider

A3S provider bundle for the [Vercel AI SDK](https://ai-sdk.dev/).
It exposes the A3S OpenAI-compatible entrypoints and dynamically routes Anthropic and Gemini model ids to their A3S upstream equivalents.

## Installation

```bash
npm install ai @cherrydesktop/ai-sdk-provider @ai-sdk/anthropic @ai-sdk/google @ai-sdk/openai
# or
pnpm add ai @cherrydesktop/ai-sdk-provider @ai-sdk/anthropic @ai-sdk/google @ai-sdk/openai
```

> **Note**: This package requires peer dependencies `ai`, `@ai-sdk/anthropic`, `@ai-sdk/google`, and `@ai-sdk/openai` to be installed.

## Usage

```ts
import { createA3s, a3s } from '@cherrydesktop/ai-sdk-provider'

const a3sProvider = createA3s({
 apiKey: process.env.A3S_API_KEY,
 // optional overrides:
 // baseURL: 'http://127.0.0.1:8080/v1',
 // anthropicBaseURL: 'http://127.0.0.1:8080/anthropic',
 // geminiBaseURL: 'http://127.0.0.1:8080/gemini/v1beta',
})

// Chat models will auto-route based on the model id prefix:
const openaiModel = a3sProvider.chat('gpt-4o-mini')
const anthropicModel = a3sProvider.chat('claude-3-5-sonnet-latest')
const geminiModel = a3sProvider.chat('gemini-2.0-pro-exp')

const { text } = await openaiModel.invoke('Hello A3S!')
```

The provider also exposes `completion`, `responses`, `embedding`, `image`, `transcription`, and `speech` helpers aligned with the upstream APIs.

See [AI SDK docs](https://ai-sdk.dev/providers/community-providers/custom-providers) for configuring custom providers.
