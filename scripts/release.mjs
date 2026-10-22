// Builds a signed release and the latest.json the in-app updater reads.
//
//   npm run release -- --notes "What changed"            build into target/release/bundle/release
//   npm run release -- --notes "What changed" --publish  ...and create the GitHub release
//
// Bump "version" in src-tauri/tauri.conf.json (and Cargo.toml / package.json) first.
// The signing key defaults to ~/.tauri/youtube-glass.key; override with TAURI_SIGNING_PRIVATE_KEY.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "lukassaridis/youtube-glass";
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const publish = args.includes("--publish");
const notesAt = args.indexOf("--notes");
const config = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const version = config.version;
const notes = notesAt >= 0 ? args[notesAt + 1] : `YouTube Glass ${version}`;
const tag = `v${version}`;

// One command string through the shell, since npx and gh are .cmd shims on Windows.
function run(command, env = process.env) {
  const result = spawnSync(command, { cwd: root, env, stdio: "inherit", shell: true });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

const env = { ...process.env };
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  const keyPath = join(homedir(), ".tauri", "youtube-glass.key");
  if (!existsSync(keyPath)) {
    console.error(`Signing key not found at ${keyPath}`);
    process.exit(1);
  }
  env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyPath, "utf8");
}
env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";

run("npx tauri build", env);

const releaseDir = join(root, "src-tauri", "target", "release");
const nsisDir = join(releaseDir, "bundle", "nsis");
const setup = join(nsisDir, `${config.productName}_${version}_x64-setup.exe`);
// GitHub turns spaces in asset names into dots; name it that way up front so the URL is stable.
const setupAsset = `${config.productName.replace(/ /g, ".")}_${version}_x64-setup.exe`;
const outDir = join(releaseDir, "bundle", "release");
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
copyFileSync(setup, join(outDir, setupAsset));
copyFileSync(join(releaseDir, "youtube-glass.exe"), join(outDir, "youtube-glass.exe"));

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(`${setup}.sig`, "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/${tag}/${setupAsset}`,
    },
  },
};
writeFileSync(join(outDir, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`\nRelease files for ${tag} are in ${outDir}`);

if (publish) {
  const files = [setupAsset, "youtube-glass.exe", "latest.json"].map((file) => `"${join(outDir, file)}"`);
  run(
    `gh release create ${tag} ${files.join(" ")} --repo ${REPO} --title "YouTube Glass ${version}" --notes ${JSON.stringify(notes)}`
  );
}
