import * as fs from 'fs'
import * as path from 'path'

import { checkTranslationValues } from './i18n-check-values'
import { sortedObjectByKeys } from './sort'

const baseLocale = process.env.TRANSLATION_BASE_LOCALE ?? 'en-us'
const baseFileName = `${baseLocale}.json`

const rendererLocalesDir = path.join(__dirname, '../src/renderer/i18n/locales')

/** Catalogs are flat: every key is a dotted path mapping straight to its translated string. */
type I18N = { [key: string]: string }

/**
 * 检查目标文件与基准模板的键集合是否完全一致（缺键、多键、值不是字符串都会抛错）。
 */
function checkKeys(target: I18N, template: I18N): void {
  for (const key in template) {
    if (!(key in target)) {
      throw new Error(`缺少属性 ${key}`)
    }
    if (typeof target[key] !== 'string') {
      throw new Error(`属性 ${key} 不是字符串，catalog 必须是扁平的 key -> string`)
    }
  }

  for (const targetKey in target) {
    if (!(targetKey in template)) {
      throw new Error(`多余属性 ${targetKey}`)
    }
  }
}

function isSortedI18N(obj: I18N): boolean {
  return JSON.stringify(obj) === JSON.stringify(sortedObjectByKeys(obj))
}

function readI18N(filePath: string): I18N {
  if (!fs.existsSync(filePath)) {
    throw new Error(`文件 ${filePath} 不存在，请检查路径或文件名`)
  }
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf-8'))
  } catch (error) {
    throw new Error(`解析 ${filePath} 出错。${error}`)
  }
}

/**
 * 校验一组翻译文件：基准模板有序，其余文件有序且与基准键集合完全一致。
 *
 * @param label 分组名（用于报错信息，如 renderer / main）
 * @param baseFilePath 基准模板文件
 * @param files 该组需要校验的全部翻译文件（含基准模板本身）
 */
function checkCatalog(label: string, baseFilePath: string, files: string[]): I18N {
  const baseJson = readI18N(baseFilePath)

  if (!isSortedI18N(baseJson)) {
    throw new Error(`[${label}] 主模板 ${path.basename(baseFilePath)} 的键值未按字典序排序。`)
  }

  for (const filePath of files) {
    if (path.resolve(filePath) === path.resolve(baseFilePath)) continue
    const targetJson = readI18N(filePath)
    if (!isSortedI18N(targetJson)) {
      throw new Error(`[${label}] 翻译文件 ${path.basename(filePath)} 的键值未按字典序排序。`)
    }
    try {
      checkKeys(targetJson, baseJson)
    } catch (e) {
      console.error(e)
      throw new Error(`[${label}] 在检查 ${filePath} 时出错`)
    }
  }

  return baseJson
}

function listJsonFiles(dir: string): string[] {
  return fs
    .readdirSync(dir)
    .filter((file) => file.endsWith('.json'))
    .map((file) => path.join(dir, file))
}

function checkTranslations(): void {
  checkCatalog('renderer', path.join(rendererLocalesDir, baseFileName), listJsonFiles(rendererLocalesDir))
}

export function main() {
  try {
    checkTranslations()
    const { checked, failures } = checkTranslationValues()
    if (failures.length > 0) {
      for (const failure of failures) console.error(`  x ${failure}`)
      throw new Error(`${failures.length} translations failed validation`)
    }
    console.log(`i18n 检查已通过（已校验 ${checked} 条翻译）`)
  } catch (e) {
    console.error(e)
    throw new Error(`检查未通过。请修复上面的 i18n 问题。`)
  }
}

main()
