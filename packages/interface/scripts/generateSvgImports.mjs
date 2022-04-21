import * as fs from 'fs/promises';
import * as path from 'path';

(async function main() {
  async function exists(path) {
    try {
      await fs.access(path);
      return true;
    } catch {
      return false;
    }
  }

  const files = await fs.readdir('./src/assets/icons');
  const icons = files.filter((f) => f.endsWith('.svg'));
  let str = '';

  for (let binding of icons) {
    let name = binding.split('.')[0];
    str += `import { ReactComponent as ${
      name.charAt(0).toUpperCase() + name.slice(1)
    } } from './${name}.svg';\n`;
  }
  str += '\n\nexport default {\n';

  for (let binding of icons) {
    let name = binding.split('.')[0];
    let componentName = name.charAt(0).toUpperCase() + name.slice(1);
    str += `  ${name}: ${componentName},\n`;
  }

  str += '}\n';

  let outPath = './src/assets/icons/index.ts';

  let indexExists = await exists(outPath);

  if (indexExists) {
    await fs.rm(outPath);
  }

  await fs.writeFile(outPath, str);
})();
