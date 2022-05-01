import fs from 'fs';
import type { Config } from '@svgr/core';
import { transformWithEsbuild } from 'vite';
import type { Plugin } from 'vite';

type Options = {
  svgrOptions?: Config;
  esbuildOptions?: Parameters<typeof transformWithEsbuild>[2];
};

export default function svgrPlugin({ svgrOptions, esbuildOptions }: Options = {}): Plugin {
  return {
    name: 'vite:svgr',
    async transform(code, id) {
      if (id.endsWith('.svg')) {
        const { transform: convert } = await import('@svgr/core');

        const svgCode = await fs.promises.readFile(id, 'utf8');

        const componentCode = await convert(
          svgCode,
          {
            ...svgrOptions,
            plugins: [...(svgrOptions?.plugins ?? []), '@svgr/plugin-svgo', '@svgr/plugin-jsx'],
            svgoConfig: {
              ...(svgrOptions?.svgoConfig ?? {}),
              plugins: [
                {
                  name: 'preset-default',
                  params: {
                    overrides: {
                      inlineStyles: {
                        onlyMatchedOnce: false
                      }
                    }
                  }
                },
                'removeStyleElement'
              ]
            }
          },
          {
            componentName: 'ReactComponent',
            filePath: id
          }
        ).then((res) => {
          return res.replace('export default ReactComponent', `export { ReactComponent }`);
        });

        const res = await transformWithEsbuild(componentCode + '\n' + code, id, {
          loader: 'jsx',
          ...esbuildOptions
        });

        return {
          code: res.code,
          map: null
        };
      }
    }
  };
}
