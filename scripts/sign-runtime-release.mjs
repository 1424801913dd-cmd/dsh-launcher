import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign, verify } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const DOMAIN = Buffer.from("dsh-runtime-bundle-v1\0", "utf8");

function argumentsMap(values) {
  const result = new Map();
  for (let index = 0; index < values.length; index += 2) {
    const key = values[index];
    const value = values[index + 1];
    if (!key?.startsWith("--") || value === undefined) throw new Error(`Invalid argument: ${key ?? ""}`);
    result.set(key.slice(2), value);
  }
  return result;
}

function required(args, name) {
  const value = args.get(name);
  if (!value) throw new Error(`Missing --${name}.`);
  return value;
}

async function writeSecure(path, bytes) {
  const absolute = resolve(path);
  await mkdir(dirname(absolute), { recursive: true });
  await writeFile(absolute, bytes, { flag: "wx", mode: 0o600 });
}

async function generate(args) {
  const privateOutput = required(args, "private-output");
  const publicOutput = required(args, "public-output");
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const privateDer = privateKey.export({ type: "pkcs8", format: "der" });
  const publicDer = publicKey.export({ type: "spki", format: "der" });
  await writeSecure(privateOutput, Buffer.from(privateDer).toString("base64") + "\n");
  await writeSecure(publicOutput, Buffer.from(publicDer).subarray(-32).toString("base64") + "\n");
  process.stdout.write(`Generated Ed25519 release key. Keep ${resolve(privateOutput)} offline and secret.\n`);
}

async function signRelease(args) {
  const privateKeyValue = process.env.DSH_RUNTIME_SIGNING_PRIVATE_KEY;
  if (!privateKeyValue) throw new Error("DSH_RUNTIME_SIGNING_PRIVATE_KEY is required.");
  const privateKey = createPrivateKey({
    key: Buffer.from(privateKeyValue.trim(), "base64"),
    type: "pkcs8",
    format: "der",
  });
  const publicDer = createPublicKey(privateKey).export({ type: "spki", format: "der" });
  const publicKey = Buffer.from(publicDer).subarray(-32).toString("base64");
  if (process.env.DSH_RUNTIME_PUBLIC_KEY?.trim() && process.env.DSH_RUNTIME_PUBLIC_KEY.trim() !== publicKey) {
    throw new Error("DSH_RUNTIME_PUBLIC_KEY does not match the signing private key.");
  }

  const bundlePath = resolve(required(args, "bundle"));
  const bundle = await readFile(bundlePath);
  const bundleDigest = createHash("sha256").update(bundle).digest();
  const metadata = JSON.parse(await readFile(resolve(required(args, "metadata")), "utf8"));
  const now = Date.now();
  const sequence = Number(required(args, "sequence"));
  if (!Number.isSafeInteger(sequence) || sequence <= 0) throw new Error("--sequence must be a positive integer.");
  const release = {
    channel: metadata.channel,
    dshVersion: metadata.dshVersion,
    nodeVersion: metadata.nodeVersion,
    architecture: metadata.architecture,
    bundleUrl: required(args, "bundle-url"),
    length: bundle.length,
    sha256: bundleDigest.toString("hex"),
    signature: sign(null, Buffer.concat([DOMAIN, bundleDigest]), privateKey).toString("base64"),
    packageIntegrity: metadata.packageIntegrity,
    recipeId: metadata.recipeId,
    minLauncherVersion: args.get("min-launcher-version") ?? "0.3.0",
    migration: { required: false, id: "none" },
  };
  const releases = [release];
  const existingManifest = args.get("existing-manifest");
  if (existingManifest) {
    try {
      const existing = JSON.parse(await readFile(resolve(existingManifest), "utf8"));
      const existingPayload = Buffer.from(existing.payload, "base64");
      if (existing.keyId !== required(args, "key-id") || !verify(null, existingPayload, createPublicKey(privateKey), Buffer.from(existing.signature, "base64"))) {
        throw new Error("Existing manifest signature or keyId is invalid.");
      }
      const previous = JSON.parse(existingPayload.toString("utf8"));
      if (sequence <= previous.sequence) throw new Error("--sequence must be greater than the existing manifest sequence.");
      releases.push(...previous.releases.filter((item) => item.channel !== release.channel));
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  const payload = Buffer.from(JSON.stringify({
    schemaVersion: 1,
    sequence,
    issuedAtMs: now,
    expiresAtMs: now + Number(args.get("valid-for-ms") ?? 30 * 24 * 60 * 60 * 1000),
    releases,
  }), "utf8");
  const envelope = {
    schemaVersion: 1,
    keyId: required(args, "key-id"),
    payload: payload.toString("base64"),
    signature: sign(null, payload, privateKey).toString("base64"),
  };
  const manifestOutput = resolve(required(args, "manifest-output"));
  await mkdir(dirname(manifestOutput), { recursive: true });
  await writeFile(manifestOutput, JSON.stringify(envelope, null, 2) + "\n", { flag: "w" });

  const configOutput = args.get("config-output");
  if (configOutput) {
    const config = {
      schemaVersion: 1,
      enabled: true,
      manifestUrl: required(args, "manifest-url"),
      keyId: required(args, "key-id"),
      publicKey,
    };
    await writeFile(resolve(configOutput), JSON.stringify(config, null, 2) + "\n", { flag: "w" });
  }
}

const [command, ...values] = process.argv.slice(2);
const args = argumentsMap(values);
if (command === "generate") await generate(args);
else if (command === "sign") await signRelease(args);
else throw new Error("Usage: sign-runtime-release.mjs generate|sign [options]");
