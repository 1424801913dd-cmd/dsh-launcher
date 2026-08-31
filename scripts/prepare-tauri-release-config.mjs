import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const endpoint = process.env.DSH_LAUNCHER_UPDATE_ENDPOINT;
const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY;
if (!endpoint?.startsWith("https://")) throw new Error("DSH_LAUNCHER_UPDATE_ENDPOINT must be HTTPS.");
if (!publicKey?.trim()) throw new Error("TAURI_UPDATER_PUBLIC_KEY is required.");
const config = {
  bundle: { active: true, createUpdaterArtifacts: true, targets: ["nsis"] },
  plugins: {
    updater: {
      endpoints: [endpoint],
      pubkey: publicKey,
      windows: { installMode: "passive" },
    },
  },
};
await writeFile(resolve("src-tauri/tauri.release.conf.json"), JSON.stringify(config, null, 2) + "\n");
