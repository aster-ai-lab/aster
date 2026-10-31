// Vendor the official MuJoCo WASM build into ui/workbench/vendor/mujoco.
//
//   npm pack @mujoco/mujoco@3.15.0 && tar xzf mujoco-mujoco-3.15.0.tgz
//   node tools/vendor-mujoco.mjs package
//
// mujoco.wasm is unmodified. It is gzip-compressed and carried as base64 in
// wasm.js: the view policy has connect-src 'none', so the page cannot fetch a
// binary asset, and the API admits files up to 8 MiB.
//
// mujoco.js gets two patches. The upstream build generates embind invokers
// with `new Function`, which the view policy refuses ('wasm-unsafe-eval' only,
// no 'unsafe-eval'). Each patch replaces one generator with a closure that does
// the same calls in the same order, as Emscripten's DYNAMIC_EXECUTION=0 build
// does. The script refuses an input where an anchor is missing or repeated.
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { gzipSync } from 'node:zlib'

const PATCHES = [
  {
    anchor: `      let [args, invokerFnBody] = createJsInvoker(argTypes, isClassMethodFunc, returns, isAsync);
      var invokerFn = new Function(...args, invokerFnBody)(...closureArgs);
`,
    replacement: `      // aster patch: closure invoker, not generated code (view policy has no 'unsafe-eval').
      var invokerFn = function(...callArgs) {
        checkArgCount(callArgs.length, minArgs, expectedArgCount, humanName, throwBindingError);
        var destructors = needsDestructorStack ? [] : null;
        var wired = [cppTargetFunc];
        var thisWired;
        if (isClassMethodFunc) {
          thisWired = argTypes[1]['toWireType'](destructors, this);
          wired.push(thisWired);
        }
        var argsWired = new Array(expectedArgCount);
        for (var i = 0; i < expectedArgCount; ++i) {
          argsWired[i] = argTypes[i + 2]['toWireType'](destructors, callArgs[i]);
          wired.push(argsWired[i]);
        }
        var rv = cppInvokerFunc(...wired);
        if (needsDestructorStack) {
          runDestructors(destructors);
        } else {
          for (var i = isClassMethodFunc ? 1 : 2; i < argTypes.length; ++i) {
            if (argTypes[i].destructorFunction !== null) {
              argTypes[i].destructorFunction(i === 1 ? thisWired : argsWired[i - 2]);
            }
          }
        }
        if (returns) return argTypes[0]['fromWireType'](rv);
      };
`
  },
  {
    anchor: `      var invokerFunction = new Function(...params, functionBody)(...args);
`,
    replacement: `      // aster patch: closure method caller, not generated code (view policy has no 'unsafe-eval').
      var invokerFunction = (obj, func, destructorsRef, argsPointer) => {
        var values = kind === /* FUNCTION */ 0 ? [obj] : [];
        var offset = 0;
        for (var i = 0; i < argCount; ++i) {
          values.push(types[i].readValueFromPointer(argsPointer + offset));
          offset += types[i].argPackAdvance;
        }
        var rv = kind === /* CONSTRUCTOR */ 1 ? Reflect.construct(func, values) : func.call(...values);
        if (!retType.isVoid) return emval_returnValue(retType, destructorsRef, rv);
      };
`
  }
]

const source = process.argv[2]
if (!source) {
  console.error('usage: node tools/vendor-mujoco.mjs <unpacked @mujoco/mujoco package>')
  process.exit(2)
}
const target = new URL('../ui/workbench/vendor/mujoco/', import.meta.url)
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')

const upstream = readFileSync(join(source, 'mujoco.js'), 'utf8')
let script = upstream
for (const { anchor, replacement } of PATCHES) {
  const count = script.split(anchor).length - 1
  if (count !== 1) throw new Error(`patch anchor found ${count} times:\n${anchor}`)
  script = script.replace(anchor, replacement)
}
if (/new Function|\beval\(/.test(script)) throw new Error('mujoco.js still generates code at run time')
writeFileSync(new URL('mujoco.js', target), script)

const wasm = readFileSync(join(source, 'mujoco.wasm'))
const packed = gzipSync(wasm, { level: 9 }).toString('base64')
writeFileSync(new URL('wasm.js', target), `export default ${JSON.stringify(packed)}\n`)

console.log(`upstream mujoco.js   ${digest(upstream)}`)
console.log(`shipped  mujoco.js   ${digest(script)}`)
console.log(`upstream mujoco.wasm ${digest(wasm)}`)
