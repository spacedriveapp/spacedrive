import { queryClient, rspcClient } from './utils/rspc';

function test(fn: () => Promise<void>) {
  return async () => {
    await fn();
    queryClient.invalidateQueries();
  };
}

const wait = (ms: number) => new Promise((res) => setTimeout(res, ms));

export const tests = {
  three: {
    name: 'Three',
    run: test(async () => {
      const [db1, db2, db3] = await Promise.all([
        rspcClient.mutation(['createDatabase', ' ']),
        rspcClient.mutation(['createDatabase', ' ']),
        rspcClient.mutation(['createDatabase', ' '])
      ]);

      const dbs = await rspcClient.query(['dbs', 'cringe']);

      for (const db of dbs) {
        await rspcClient.mutation(['file_path.create', db]);
      }

      for (const db of dbs) {
        await rspcClient.mutation(['pullOperations', db]);
      }

      await rspcClient.mutation(['file_path.create', dbs[0]]);
      await rspcClient.mutation(['file_path.create', dbs[0]]);

      for (const db of dbs) {
        await rspcClient.mutation(['pullOperations', db]);
      }

      await rspcClient.mutation(['pullOperations', dbs[1]]);
      await rspcClient.mutation(['pullOperations', dbs[1]]);
      await rspcClient.mutation(['pullOperations', dbs[1]]);
      await rspcClient.mutation(['pullOperations', dbs[1]]);
    })
  }
};
