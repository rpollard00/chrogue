// A TypeScriptToLua plugin for the Chrogue rules. It handles three things that the standard transpiler gets wrong
// or does not support for this code. Each fix comes from the TypeScript type, thus it also applies to new code.
//
// 1. Object.hasOwn(a, b): not supported. It becomes rawget(a, b) ~= nil.
// 2. for (const x of array) where the items can be null or undefined (the board): ipairs stops at the first nil.
//    The loop uses __chrogue_sparse_ipairs, which goes to the largest index.
// 3. f(...array) where the items can be null or undefined: unpack stops at a nil.
//    The call uses __chrogue_sparse_unpack.
// rules_runtime.lua defines the two functions.
const ts = require('typescript');
const tstl = require('typescript-to-lua');

function itemsCanBeNil(context, expression) {
  const checker = context.checker;
  const type = checker.getTypeAtLocation(expression);
  if (!checker.isArrayType(type)) return false;
  const [item] = checker.getTypeArguments(type);
  const parts = item.isUnion() ? item.types : [item];
  return parts.some((part) => part.flags & (ts.TypeFlags.Null | ts.TypeFlags.Undefined));
}

function rename(call, from, to) {
  if (tstl.isCallExpression(call) && tstl.isIdentifier(call.expression) && call.expression.text === from) {
    call.expression = tstl.createIdentifier(to);
    return true;
  }
  return false;
}

module.exports = {
  visitors: {
    [ts.SyntaxKind.CallExpression]: (node, context) => {
      const callee = node.expression;
      const hasOwn = ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression)
        && callee.expression.text === 'Object' && callee.name.text === 'hasOwn';
      if (hasOwn && node.arguments.length === 2) {
        const args = node.arguments.map((arg) => context.transformExpression(arg));
        return tstl.createParenthesizedExpression(tstl.createBinaryExpression(
          tstl.createCallExpression(tstl.createIdentifier('rawget'), args),
          tstl.createNilLiteral(), tstl.SyntaxKind.InequalityOperator, node));
      }
      return context.superTransformExpression(node);
    },
    [ts.SyntaxKind.ForOfStatement]: (node, context) => {
      const statements = context.superTransformStatements(node);
      if (!itemsCanBeNil(context, node.expression)) return statements;
      const loop = statements.find((statement) => tstl.isForInStatement(statement));
      if (!loop || !rename(loop.expressions[0], 'ipairs', '__chrogue_sparse_ipairs')) {
        throw new Error(`chrogue plugin: cannot fix the loop over a sparse array at ${node.getSourceFile().fileName}:${node.getStart()}`);
      }
      return statements;
    },
    [ts.SyntaxKind.SpreadElement]: (node, context) => {
      const result = context.superTransformExpression(node);
      if (!itemsCanBeNil(context, node.expression)) return result;
      if (!rename(result, 'unpack', '__chrogue_sparse_unpack')) {
        throw new Error(`chrogue plugin: cannot fix the spread of a sparse array at ${node.getSourceFile().fileName}:${node.getStart()}`);
      }
      return result;
    },
  },
};
