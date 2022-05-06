import * as fs from 'fs/promises';
import * as path from 'path';

(async function main() {
  async function exists(path: string) {
    try {
      await fs.access(path);
      return true;
    } catch {
      return false;
    }
  }

  const files = await fs.readdir(path.join(__dirname, '../bindings'));
  const bindings = files.filter((f) => f.endsWith('.ts'));
  let str = '';
  // str += `export * from './types';\n`;

  for (const binding of bindings) {
    str += `export * from './bindings/${binding.split('.')[0]}';\n`;
  }

  const indexExists = await exists(path.join(__dirname, '../index.ts'));

  if (indexExists) {
    await fs.rm(path.join(__dirname, '../index.ts'));
  }

  await fs.writeFile(path.join(__dirname, '../index.ts'), str);
})();
