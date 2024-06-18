import which from 'which'

import * as os from 'os';

import * as vscode from 'vscode';
import * as fs from 'fs';
import * as stream from 'stream';
import * as child_process from 'child_process'
import path from "path";
import { promisify } from "util";

let githubReleaseURL = 'https://api.github.com/repos/Decodetalkers/neocmakelsp/releases/latest';

export namespace Github {
  export interface Release {
    name: string, tag_name: string, assets: Array<Asset>,
  }

  export interface Asset {
    name: string, browser_download_url: string,
  }

  export async function isLastestRelease(path: string, abort: AbortController) {
    let latestversion = await lastestRelease(abort);
    let version = await getNeocmakeVersion(path);
    return latestversion.tag_name.substring(1) === version
  }

  export async function lastestRelease(timeoutController: AbortController) {
    const timeout = setTimeout(() => { timeoutController.abort(); }, 5000)
    try {
      const response = await fetch(githubReleaseURL, { signal: timeoutController.signal })
      if (!response.ok) {
        console.log(response.url, response.status, response.statusText);
        throw new Error(`Can't fetch release: ${response.statusText}`)
      }
      return await response.json() as Release;
    } catch (e) {
      throw e;
    } finally {
      clearTimeout(timeout)
    }
  }

  export async function getNeocmakeLspPath(path: string) {
    try {
      return await which(path);
    } catch (_) {
      return undefined
    }
  }

  export async function getNeocmakeVersion(path: string) {
    if (await getNeocmakeLspPath(path) === undefined) {
      return undefined
    }
    const output = await run(path, ['--version']);
    const version = output.split(' ')[1].trimEnd()
    return version
  }

  async function run(command: string, flags: string[]): Promise<string> {
    const child = child_process.spawn(command, flags, { stdio: ['ignore', "pipe", 'ignore'] });
    let output = '';
    for await (const chunk of child.stdout)
      output += chunk;
    return output
  }

}

namespace Install {
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
      if (await Github.isLastestRelease(exePath, abort)) {
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
}

function targetName() {
  let arch = os.arch()
  switch (os.platform()) {
    case "win32":
      return "neocmakelsp-x86_64-pc-windows-msvc.exe"
    case "darwin":
      if (arch == "x64") {
        return "neocmakelsp-x86_64-apple-darwin";
      } else {
        return "neocmakelsp-aarch64-apple-darwin";
      }
    case "linux":
      return "neocmakelsp-x86_64-unknown-linux-gnu"
    default:
      return undefined;
  }
}

function getGithubAssert(asserts: Github.Asset[]) {
  let target = targetName();
  if (target === undefined) {
    return undefined;
  }
  return asserts.find(assert => assert.name === target);
}

export async function installLatestNeocmakeLsp(path: string) {
  let timeoutController = new AbortController();
  try {
    const latestRe = await Github.lastestRelease(timeoutController);
    let assert = getGithubAssert(latestRe.assets);
    if (assert === undefined) {
      console.log("Your platform is not supported");
      return undefined;
    }
    return await Install.install(assert, timeoutController, path);
  } catch (e) {
    console.log(`Error: ${e}`);
    return undefined;
  }
}
