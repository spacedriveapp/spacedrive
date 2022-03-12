import * as fs from 'fs/promises';
import * as path from 'path';

(async function main() {
  const files = await fs.readdir(path.join(__dirname, '../bindings'));
  const bindings = files.filter((f) => f.endsWith('.ts'));
  let str = '';
  // str += `export * from './types';\n`;

  for (let binding of bindings) {
    str += `export * from './bindings/${binding.split('.')[0]}';\n`;
  }

  await fs.writeFile(path.join(__dirname, '../index.ts'), str);
})();
