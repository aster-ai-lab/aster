import {
  createCssVariablesTheme,
  createSingletonShorthands,
  getTokenStyleObject,
  guessEmbeddedLanguages,
  normalizeTheme,
  stringifyTokenStyle
} from 'shiki/core'
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript'
import { createOnigurumaEngine } from 'shiki/engine/oniguruma'

import {
  bundledLanguages,
  bundledLanguagesInfo,
  bundledThemes,
  bundledThemesInfo,
  createHighlighter
} from './shikiBundle'

export {
  bundledLanguages,
  bundledLanguagesInfo,
  bundledThemes,
  bundledThemesInfo,
  createCssVariablesTheme,
  createHighlighter,
  createJavaScriptRegexEngine,
  createOnigurumaEngine,
  getTokenStyleObject,
  normalizeTheme,
  stringifyTokenStyle
}

const shorthands = /* @__PURE__ */ createSingletonShorthands(createHighlighter, {
  guessEmbeddedLanguages
})

export const {
  codeToHast,
  codeToHtml,
  codeToTokens,
  codeToTokensBase,
  codeToTokensWithThemes,
  getLastGrammarState,
  getSingletonHighlighter
} = shorthands
