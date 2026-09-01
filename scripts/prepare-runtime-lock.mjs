import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const args = process.argv.slice(2);

function argument(name) {
  const index = args.indexOf(name);
  if (index < 0 || !args[index + 1]) throw new Error(`Missing required argument: ${name}`);
  return args[index + 1];
}

function optionalArgument(name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

const projectRoot = resolve(argument("--project-root"));
const checkPath = optionalArgument("--check-lock");
const sourcePath = resolve(checkPath ?? argument("--source-lock"));
const outputPath = checkPath ? null : resolve(argument("--output"));
const dshVersion = argument("--dsh-version");
const recipes = readJson(resolve(projectRoot, "src-tauri/resources/compatibility-recipes.json"));
const recipe = recipes.recipes.find((item) => item.dshVersion === dshVersion);
if (!recipe) throw new Error(`No compatibility recipe found for DSH ${dshVersion}.`);

const lock = readJson(sourcePath);
if (lock.lockfileVersion !== 3 || !lock.packages || !lock.packages[""]) {
  throw new Error("Source lock must use npm lockfileVersion 3 and contain a root package.");
}
const dsh = lock.packages["node_modules/@deepseek-ai/dsh"];
if (!dsh || dsh.version !== dshVersion || dsh.integrity !== recipe.packageIntegrity) {
  throw new Error("Source lock DSH version or integrity does not match the compatibility recipe.");
}

const drift = Object.entries(lock.packages)
  .map(([path, metadata]) => {
    const marker = "node_modules/";
    const index = path.lastIndexOf(marker);
    return {
      path,
      metadata,
      name: metadata.name ?? (index >= 0 ? path.slice(index + marker.length) : path),
    };
  })
  .filter((item) => item.name.startsWith("@deepseek-ai/dsh-") && item.metadata.version !== dshVersion);
if (drift.length > 0) {
  throw new Error(`Source lock contains drifting internal DSH packages:\n${drift.map((item) => `${item.name}@${item.metadata.version}`).join("\n")}`);
}

const dependencies = {
  "@deepseek-ai/dsh": dshVersion,
  ...(recipe.supplementalDependencies ?? {}),
};
if (checkPath) {
  const root = lock.packages[""];
  const expectedEntries = Object.entries(dependencies).sort(([left], [right]) => left.localeCompare(right));
  const actualEntries = Object.entries(root.dependencies ?? {}).sort(([left], [right]) => left.localeCompare(right));
  if (
    lock.name !== "dsh-launcher-runtime-bundle" ||
    lock.version !== "1.0.0" ||
    root.name !== lock.name ||
    root.version !== lock.version ||
    JSON.stringify(actualEntries) !== JSON.stringify(expectedEntries)
  ) {
    throw new Error("Reviewed Runtime lock root dependencies do not exactly match the compatibility recipe.");
  }
  console.log(`Reviewed Runtime lock valid: ${sourcePath}`);
  console.log(`DSH ${dshVersion}; packages=${Object.keys(lock.packages).length - 1}; internal drift=0`);
  process.exit(0);
}

lock.name = "dsh-launcher-runtime-bundle";
lock.version = "1.0.0";
lock.packages[""] = {
  name: lock.name,
  version: lock.version,
  dependencies,
};

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(lock, null, 2)}\n`, "utf8");
console.log(`Prepared reviewed Runtime lock: ${outputPath}`);
console.log(`DSH ${dshVersion}; packages=${Object.keys(lock.packages).length - 1}; internal drift=0`);
