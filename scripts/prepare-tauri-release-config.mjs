import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const endpoint = process.env.DSH_LAUNCHER_UPDATE_ENDPOINT;
const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY;
if (!endpoint?.startsWith("https://")) throw new Error("DSH_LAUNCHER_UPDATE_ENDPOINT must be HTTPS.");
if (!publicKey?.trim()) throw new Error("TAURI_UPDATER_PUBLIC_KEY is required.");
const config = {
  // SignPath must sign the final NSIS executable before the Tauri updater
  // archive and Ed25519 signature are created. The release workflow creates
  // those updater artifacts after both Authenticode signing requests finish.
  bundle: { active: true, createUpdaterArtifacts: false, targets: ["nsis"] },
  plugins: {
    updater: {
      endpoints: [endpoint],
      pubkey: publicKey,
      windows: { installMode: "passive" },
    },
  },
};
await writeFile(resolve("src-tauri/tauri.release.conf.json"), JSON.stringify(config, null, 2) + "\n");
