import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

function readOption(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) throw new Error(`${name} is required.`);
  return process.argv[index + 1];
}

const version = readOption("--version");
const repository = readOption("--repository");
const archiveName = readOption("--archive-name");
const signaturePath = resolve(readOption("--signature"));
const notesPath = resolve(readOption("--notes"));
const outputPath = resolve(readOption("--output"));

if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`Invalid release version: ${version}`);
}
if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
  throw new Error(`Invalid GitHub repository: ${repository}`);
}
if (archiveName.includes("/") || archiveName.includes("\\")) {
  throw new Error("--archive-name must be a file name, not a path.");
}

const signature = (await readFile(signaturePath, "utf8")).trim();
const notes = await readFile(notesPath, "utf8");
if (!signature) throw new Error("The Tauri updater signature is empty.");

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature,
      url: `https://github.com/${repository}/releases/latest/download/${encodeURIComponent(archiveName)}`,
    },
  },
};

await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
