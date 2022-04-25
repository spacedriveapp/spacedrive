import * as cache from "@actions/cache";
import * as core from "@actions/core";
import { cleanTarget, getCacheConfig, getCargoBins, getPackages, stateBins, stateKey } from "./common";

async function run() {
  if (!cache.isFeatureAvailable()) {
    setCacheHitOutput(false);
    return;
  }

  try {
    var cacheOnFailure = core.getInput("cache-on-failure").toLowerCase();
    if (cacheOnFailure !== "true") {
      cacheOnFailure = "false";
    }
    core.exportVariable("CACHE_ON_FAILURE", cacheOnFailure);
    core.exportVariable("CARGO_INCREMENTAL", 1);

    const { paths, key, restoreKeys } = await getCacheConfig();

    const bins = await getCargoBins();
    core.saveState(stateBins, JSON.stringify([...bins]));

    core.info(`Restoring paths:\n    ${paths.join("\n    ")}`);
    core.info(`In directory:\n    ${process.cwd()}`);
    core.info(`Using keys:\n    ${[key, ...restoreKeys].join("\n    ")}`);
    const restoreKey = await cache.restoreCache(paths, key, restoreKeys);
    if (restoreKey) {
      core.info(`Restored from cache key "${restoreKey}".`);
      core.saveState(stateKey, restoreKey);

      if (restoreKey !== key) {
        // pre-clean the target directory on cache mismatch
        const packages = await getPackages();

        await cleanTarget(packages);
      }

      setCacheHitOutput(restoreKey === key);
    } else {
      core.info("No cache found.");

      setCacheHitOutput(false);
    }
  } catch (e) {
    setCacheHitOutput(false);

    core.info(`[warning] ${(e as any).message}`);
  }
}

function setCacheHitOutput(cacheHit: boolean): void {
  core.setOutput("cache-hit", cacheHit.toString());
}

run();
