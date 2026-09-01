import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const args = process.argv.slice(2);

function argument(name, fallback = undefined) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : fallback;
}

function hasFlag(name) {
  return args.includes(name);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function packageName(path, metadata) {
  if (metadata.name) return metadata.name;
  const marker = "node_modules/";
  const index = path.lastIndexOf(marker);
  return index >= 0 ? path.slice(index + marker.length) : path;
}

function npmPackages(lockPath, productionOnly) {
  const lock = readJson(lockPath);
  return Object.entries(lock.packages ?? {})
    .filter(([path, metadata]) => path && (!productionOnly || !metadata.dev))
    .map(([path, metadata]) => ({
      name: packageName(path.replaceAll("\\", "/"), metadata),
      version: metadata.version ?? "UNKNOWN",
      license: metadata.license ?? "UNKNOWN",
      path,
    }))
    .sort(comparePackages);
}

function installedNpmPackages(lockPath, installRoot, productionOnly) {
  return npmPackages(lockPath, productionOnly).filter((pkg) =>
    existsSync(join(installRoot, pkg.path)),
  );
}

function cargoPackages(projectRoot) {
  const cargo = process.env.CARGO || "cargo";
  const result = spawnSync(
    cargo,
    [
      "metadata",
      "--manifest-path",
      join(projectRoot, "src-tauri", "Cargo.toml"),
      "--format-version",
      "1",
      "--locked",
      "--offline",
      "--filter-platform",
      "x86_64-pc-windows-msvc",
    ],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(`cargo metadata failed:\n${result.stderr || result.stdout}`);
  }
  const metadata = JSON.parse(result.stdout);
  const resolved = new Set((metadata.resolve?.nodes ?? []).map((node) => node.id));
  return metadata.packages
    .filter((pkg) => resolved.has(pkg.id) && pkg.source !== null)
    .map((pkg) => ({
      name: pkg.name,
      version: pkg.version,
      license: pkg.license ?? (pkg.license_file ? `LicenseRef:${pkg.license_file}` : "UNKNOWN"),
      source: pkg.source,
      repository: pkg.repository ?? null,
    }))
    .sort(comparePackages);
}

function comparePackages(left, right) {
  return (
    left.name.localeCompare(right.name) ||
    left.version.localeCompare(right.version) ||
    (left.path ?? "").localeCompare(right.path ?? "")
  );
}

function licenseCounts(packages) {
  return Object.fromEntries(
    [...packages.reduce((counts, pkg) => {
      counts.set(pkg.license, (counts.get(pkg.license) ?? 0) + 1);
      return counts;
    }, new Map())].sort(([left], [right]) => left.localeCompare(right)),
  );
}

const reviewPattern = /(?:^|[^A-Z])(?:A?GPL|LGPL|MPL|EPL|CDDL|SSPL|BUSL)(?:[^A-Z]|$)|COMMONS[ -]CLAUSE/i;

function reviewRequired(ecosystem, packages) {
  return packages
    .filter((pkg) => reviewPattern.test(pkg.license))
    .map((pkg) => {
      const sourceUrl = ecosystem === "cargo"
        ? `https://crates.io/crates/${encodeURIComponent(pkg.name)}/${encodeURIComponent(pkg.version)}`
        : pkg.name === "@img/sharp-win32-x64"
          ? "https://github.com/lovell/sharp-libvips"
          : null;
      const documented = pkg.license === "MPL-2.0" && sourceUrl !== null;
      return {
        ecosystem,
        name: pkg.name,
        version: pkg.version,
        license: pkg.license,
        sourceUrl,
        disposition: documented ? "documented-source-availability" : "manual-review-required",
      };
    });
}

function markdownTable(packages) {
  const rows = ["| Package | Version | Declared license |", "| --- | --- | --- |"];
  for (const pkg of packages) {
    rows.push(`| ${pkg.name.replaceAll("|", "\\|")} | ${pkg.version} | ${pkg.license.replaceAll("|", "\\|")} |`);
  }
  return rows.join("\n");
}

function reviewMarkdownTable(packages) {
  const rows = [
    "| Package | Version | Declared license | Disposition | Source / evidence |",
    "| --- | --- | --- | --- | --- |",
  ];
  for (const pkg of packages) {
    const source = pkg.sourceUrl ? `[source](${pkg.sourceUrl})` : "—";
    rows.push(`| ${pkg.name.replaceAll("|", "\\|")} | ${pkg.version} | ${pkg.license.replaceAll("|", "\\|")} | ${pkg.disposition} | ${source} |`);
  }
  return rows.join("\n");
}

function writeText(path, content) {
  const absolute = resolve(path);
  mkdirSync(dirname(absolute), { recursive: true });
  writeFileSync(absolute, `${content.trimEnd()}\n`, "utf8");
  return absolute;
}

const projectRoot = resolve(argument("--project-root", process.cwd()));
const reportPath = argument("--report");
const noticePath = argument("--notice");
const checkNoticePath = argument("--check-notice");
const runtimePointer = resolve(
  argument("--runtime-pointer", "D:\\Tools\\dsh-launcher\\active.json"),
);

const runtimeOnly = hasFlag("--runtime-only");
const app = runtimeOnly ? [] : npmPackages(join(projectRoot, "package-lock.json"), true);
const cargo = runtimeOnly ? [] : cargoPackages(projectRoot);
let runtime = null;

if (existsSync(runtimePointer)) {
  const pointer = readJson(runtimePointer);
  const runtimeRoot = dirname(dirname(pointer.nodePath));
  const runtimeAppRoot = join(runtimeRoot, "app");
  const runtimeLock = join(runtimeRoot, "app", "package-lock.json");
  const nodeLicense = join(runtimeRoot, "node", "LICENSE");
  if (!existsSync(runtimeLock)) throw new Error(`Runtime package lock is missing: ${runtimeLock}`);
  const lockedPackages = npmPackages(runtimeLock, false);
  runtime = {
    id: pointer.id,
    dshVersion: pointer.dshVersion,
    nodeVersion: pointer.nodeVersion,
    nodeLicensePresent: existsSync(nodeLicense),
    lockedPackages,
    packages: installedNpmPackages(runtimeLock, runtimeAppRoot, false),
  };
} else if (hasFlag("--require-runtime")) {
  throw new Error(`Required Runtime pointer is missing: ${runtimePointer}`);
}

const missingLicenseMetadata = [
  ...app.filter((pkg) => pkg.license === "UNKNOWN").map((pkg) => ({ ecosystem: "app-npm", ...pkg })),
  ...cargo.filter((pkg) => pkg.license === "UNKNOWN").map((pkg) => ({ ecosystem: "cargo", ...pkg })),
  ...(runtime?.packages ?? [])
    .filter((pkg) => pkg.license === "UNKNOWN")
    .map((pkg) => ({ ecosystem: "runtime-npm", ...pkg })),
];
const review = [
  ...reviewRequired("app-npm", app),
  ...reviewRequired("cargo", cargo),
  ...reviewRequired("runtime-npm", runtime?.packages ?? []),
];
const unresolvedReview = review.filter((pkg) => pkg.disposition === "manual-review-required");

const report = {
  schemaVersion: 2,
  generatedAtUtc: new Date().toISOString(),
  target: "x86_64-pc-windows-msvc",
  app: { packageCount: app.length, licenseCounts: licenseCounts(app), packages: app },
  cargo: { packageCount: cargo.length, licenseCounts: licenseCounts(cargo), packages: cargo },
  runtime: runtime
    ? {
        id: runtime.id,
        dshVersion: runtime.dshVersion,
        nodeVersion: runtime.nodeVersion,
        nodeLicensePresent: runtime.nodeLicensePresent,
        lockedPackageCount: runtime.lockedPackages.length,
        omittedPlatformPackageCount: runtime.lockedPackages.length - runtime.packages.length,
        packageCount: runtime.packages.length,
        licenseCounts: licenseCounts(runtime.packages),
        packages: runtime.packages,
      }
    : null,
  missingLicenseMetadata,
  reviewRequired: review,
  unresolvedReviewRequired: unresolvedReview,
};

if (reportPath) {
  console.log(`License report: ${writeText(reportPath, JSON.stringify(report, null, 2))}`);
}

if (noticePath || checkNoticePath) {
  const runtimeSection = runtime
    ? `## Managed Runtime audited locally\n\nRuntime \`${runtime.id}\` contains Node.js ${runtime.nodeVersion} and DSH ${runtime.dshVersion}. The Node distribution's top-level license file is ${runtime.nodeLicensePresent ? "present" : "missing"}. The Runtime lock records ${runtime.lockedPackages.length} packages across supported platforms; ${runtime.packages.length} package directories are actually installed in the audited Windows x64 Runtime. Only installed packages are listed and evaluated below.\n\n${markdownTable(runtime.packages)}`
    : "## Managed Runtime audited locally\n\nNo local Runtime pointer was available. Run the audit again with `--require-runtime` before release.";
  const reviewRows = review.length
    ? reviewMarkdownTable(review)
    : "No license expressions matched the review-required policy.";
  const launcherSections = runtimeOnly
    ? ""
    : `## DSH Launcher JavaScript production dependencies\n\n${markdownTable(app)}\n\n## DSH Launcher Rust Windows dependency graph\n\n${markdownTable(cargo)}\n\n`;
  const notice = `# Third-party notices inventory

This file is generated from the locked Windows x64 dependency graph by \`scripts/license-audit.mjs\`. It records declared license metadata; it is not a legal opinion and does not replace the full license texts shipped with the corresponding components. Regenerate and review it whenever either lock file or the managed Runtime changes.

## Review-required license families

The audit flags copyleft or source-availability license families for explicit release review. A match is not automatically a violation, but the release owner must record how its notice, source-offer, relinking, or file-level obligations are met.

${reviewRows}

${launcherSections}
${runtimeSection}
`.trimEnd() + "\n";
  if (noticePath) {
    console.log(`Notice inventory: ${writeText(noticePath, notice)}`);
  }
  if (checkNoticePath) {
    const absolute = resolve(checkNoticePath);
    if (!existsSync(absolute) || readFileSync(absolute, "utf8") !== notice) {
      console.error(`Notice inventory is stale: ${absolute}`);
      process.exitCode = 1;
    } else {
      console.log(`Notice inventory is current: ${absolute}`);
    }
  }
}

console.log(
  `License audit: app=${app.length}, cargo=${cargo.length}, runtime=${runtime?.packages.length ?? "not-audited"}, missing=${missingLicenseMetadata.length}, review=${review.length}, unresolved=${unresolvedReview.length}`,
);

if (missingLicenseMetadata.length > 0 || (runtime && !runtime.nodeLicensePresent)) process.exitCode = 1;
if (hasFlag("--require-reviewed") && unresolvedReview.length > 0) process.exitCode = 1;
