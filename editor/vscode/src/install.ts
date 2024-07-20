import * as vscode from 'vscode';
import * as fs from 'fs';
import * as stream from 'stream';
import * as os from 'os';

import path from "path";
import { promisify } from "util";

import * as Github from "./github"

async function download(url: string, dest: string, abort: AbortController) {
  const response = await fetch(url, { signal: abort.signal });
  if (!response.ok) {
    throw new Error(`failed to download ${url}`);
  }
  const out = fs.createWriteStream(dest);
  await promisify(stream.pipeline)(response.body, out).catch(e => {
    fs.unlink(dest, (_) => null);
    throw e;
  });
}

export async function install(assert: Github.Asset, abort: AbortController, storagePath: string): Promise<string | undefined> {
  if (await promisify(fs.exists)(storagePath)) {
    const neocmakeExecutableName = executableName();
    const exePath = path.join(storagePath, neocmakeExecutableName);
    if (await Github.isLatestRelease(exePath, abort)) {
      return exePath;
    }
  }
  vscode.window.showInformationMessage("find new version of neocmakelsp, start downloading");
  const neocmakelspPath = path.join(storagePath, assert.name);
  const neocmakelspFinallyPath = path.join(storagePath, executableName());
  try {
    await download(assert.browser_download_url, neocmakelspPath, abort);
  } catch (_) {
    return undefined
  }

  vscode.window.showInformationMessage("neocmakelsp is downloaded");

  await fs.promises.chmod(neocmakelspPath, 0o755);
  await fs.promises.rename(neocmakelspPath, neocmakelspFinallyPath)
  return neocmakelspFinallyPath
}

function executableName(): string {
  return os.platform() == 'win32' ? 'neocmakelsp.exe' : 'neocmakelsp';
}
