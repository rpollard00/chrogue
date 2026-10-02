// The first step of `bun run build:lua`. It copies the rules of ../../src to .rules-src/ and changes the places
// where the same text has a different result in Lua. TypeScriptToLua then compiles the copy.
//
// In JavaScript, 0, NaN and '' are false in a condition. In Lua, only nil and false are false, and TypeScriptToLua
// does not change this. Thus this step puts a condition that can be a number or a string into __truthy(...).
// The step uses the types of the source, thus it also applies to code that the game gets later.
// If it finds a place that it cannot change safely, it stops with the position of that place.
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import ts from 'typescript';

const here = resolve(import.meta.dir, '..');
const srcDir = resolve(here, '../../src');
const outDir = join(here, '.rules-src');

/** The modules that the port uses. storage.ts and debug.ts are not here: they use localStorage. */
const ENTRIES = ['engine/index.ts', 'game/battle.ts', 'game/upgrades.ts', 'game/army.ts', 'game/floors.ts'];

const program = ts.createProgram(ENTRIES.map((file) => join(srcDir, file)), {
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
  lib: ['lib.es2022.d.ts'], strict: true, noEmit: true, types: [], skipLibCheck: true,
});
const checker = program.getTypeChecker();

const errors = ts.getPreEmitDiagnostics(program);
if (errors.length) {
  console.error(ts.formatDiagnosticsWithColorAndContext(errors, {
    getCanonicalFileName: (name) => name, getCurrentDirectory: () => here, getNewLine: () => '\n',
  }));
  process.exit(1);
}

// True if a value of the type can be 0, NaN, or ''. `nullable` tells if it can also be null or undefined.
function falsyPrimitive(type: ts.Type): { falsy: boolean; nullable: boolean } {
  const parts = type.isUnion() ? type.types : [type];
  let falsy = false, nullable = false;
  for (const part of parts) {
    if (part.flags & (ts.TypeFlags.Null | ts.TypeFlags.Undefined)) nullable = true;
    else if (part.isNumberLiteral()) falsy ||= part.value === 0;
    else if (part.isStringLiteral()) falsy ||= part.value === '';
    else if (part.flags & (ts.TypeFlags.NumberLike | ts.TypeFlags.StringLike)) falsy = true;
  }
  return { falsy, nullable };
}

/** The array methods whose callback gives a condition. */
const PREDICATE_METHODS = new Set(['filter', 'find', 'findIndex', 'findLast', 'findLastIndex', 'some', 'every']);

interface Insert { at: number; text: string; closing: boolean }

function prepare(file: ts.SourceFile): string {
  const inserts: Insert[] = [];
  const where = (node: ts.Node) => {
    const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
    return `${relative(here, file.fileName)}:${line + 1}:${character + 1}`;
  };
  const fail = (node: ts.Node, why: string): never => {
    throw new Error(`${where(node)}: ${why}\n  ${node.getText(file)}`);
  };

  // Puts a condition into __truthy(...) if its value can be 0 or ''.
  function condition(node: ts.Expression | undefined): void {
    if (!node) return;
    const { falsy, nullable } = falsyPrimitive(checker.getTypeAtLocation(node));
    if (!falsy) return;
    // __present has the same function. Its type keeps the narrowing of a value that can be null.
    inserts.push({ at: node.getStart(file), text: nullable ? '__present(' : '__truthy(', closing: false });
    inserts.push({ at: node.getEnd(), text: ')', closing: true });
  }

  function visit(node: ts.Node): void {
    if (ts.isIfStatement(node) || ts.isWhileStatement(node) || ts.isDoStatement(node)) condition(node.expression);
    else if (ts.isForStatement(node) || ts.isConditionalExpression(node)) condition(node.condition);
    else if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.ExclamationToken) condition(node.operand);
    else if (ts.isBinaryExpression(node)) {
      const op = node.operatorToken.kind;
      const logical = op === ts.SyntaxKind.AmpersandAmpersandToken || op === ts.SyntaxKind.BarBarToken
        || op === ts.SyntaxKind.AmpersandAmpersandEqualsToken || op === ts.SyntaxKind.BarBarEqualsToken;
      if (logical && falsyPrimitive(checker.getTypeAtLocation(node.left)).falsy) {
        fail(node, 'The left side of && or || can be 0 or "". Lua gives a different value. Compare the left side explicitly.');
      }
    } else if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression)) {
      const [callback] = node.arguments;
      const isPredicate = PREDICATE_METHODS.has(node.expression.name.text)
        && checker.isArrayLikeType(checker.getTypeAtLocation(node.expression.expression));
      if (isPredicate && callback && (ts.isArrowFunction(callback) || ts.isFunctionExpression(callback))) {
        const signature = checker.getSignatureFromDeclaration(callback);
        const result = signature && falsyPrimitive(checker.getReturnTypeOfSignature(signature));
        if (result?.falsy) {
          if (ts.isBlock(callback.body)) fail(callback, 'The callback gives a condition that can be 0 or "". Use an expression body, or compare explicitly.');
          else condition(callback.body);
        }
      }
      // Object.keys of an object literal: Lua has no key sequence, thus the keys become a list in the sequence of the source.
      if (node.expression.getText(file) === 'Object.keys' && node.arguments.length === 1) {
        const keys = literalKeys(node.arguments[0]);
        if (!keys) fail(node, 'Object.keys of a value that is not a constant object literal. Lua does not keep the key sequence.');
        else {
          inserts.push({ at: node.getStart(file), text: `(${JSON.stringify(keys)} as string[]) /* `, closing: false });
          inserts.push({ at: node.getEnd(), text: ' */', closing: true });
        }
      }
    }
    ts.forEachChild(node, visit);
  }

  function literalKeys(arg: ts.Expression): string[] | null {
    const decl = checker.getSymbolAtLocation(arg)?.valueDeclaration;
    if (!decl || !ts.isVariableDeclaration(decl) || !decl.initializer) return null;
    let init = decl.initializer;
    while (ts.isSatisfiesExpression(init) || ts.isAsExpression(init) || ts.isParenthesizedExpression(init)) init = init.expression;
    if (!ts.isObjectLiteralExpression(init)) return null;
    const keys: string[] = [];
    for (const prop of init.properties) {
      if (!prop.name || !(ts.isIdentifier(prop.name) || ts.isStringLiteral(prop.name))) return null;
      keys.push(prop.name.text);
    }
    return keys;
  }

  visit(file);
  // The inserts go in from the end of the text. At one position, a closing text comes before an opening text.
  inserts.sort((a, b) => b.at - a.at || Number(a.closing) - Number(b.closing));
  let text = file.text;
  for (const { at, text: added } of inserts) text = text.slice(0, at) + added + text.slice(at);
  return text;
}

rmSync(outDir, { recursive: true, force: true });
let changed = 0;
for (const file of program.getSourceFiles()) {
  if (file.isDeclarationFile || !file.fileName.startsWith(srcDir)) continue;
  const text = prepare(file);
  if (text !== file.text) changed++;
  const target = join(outDir, relative(srcDir, file.fileName));
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, `// Generated from src/${relative(srcDir, file.fileName)} by tools/prepare-rules.ts. Do not edit.\n${text}`);
}
writeFileSync(join(outDir, 'env.d.ts'), `// The functions of rules_runtime.lua.
declare function __truthy(value: unknown): boolean;
declare function __present<T>(value: T): value is NonNullable<T>;
`);
console.log(`prepare-rules: ${changed} files changed`);
