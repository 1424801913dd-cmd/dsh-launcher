import readline from "node:readline";
import { pathToFileURL } from "node:url";

const [, , dshEntry, ...dshArgs] = process.argv;

if (!dshEntry) {
  console.error("dsh-launcher bridge: missing DSH entry path");
  process.exit(64);
}

let started = false;
let shutdownRequested = false;

function requestShutdown() {
  shutdownRequested = true;
  const handled = process.emit("SIGTERM");
  console.error(
    handled
      ? "dsh-launcher bridge: graceful shutdown requested"
      : "dsh-launcher bridge: shutdown requested before DSH registered its handler",
  );
}

async function startDsh() {
  if (started) return;
  started = true;
  process.argv = [process.execPath, dshEntry, ...dshArgs];

  try {
    await import(pathToFileURL(dshEntry).href);
    if (shutdownRequested) requestShutdown();
  } catch (error) {
    console.error(`dsh-launcher bridge: DSH boot failed: ${error?.stack ?? error}`);
    process.exitCode = 1;
  }
}

const input = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});
input.on("line", (line) => {
  const command = line.trim();
  if (command === "start") {
    void startDsh();
  } else if (command === "shutdown") {
    requestShutdown();
  }
});
