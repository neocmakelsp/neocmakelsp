import * as os from 'os';

import * as Github from "./github";
import * as Install from "./install";

function targetName() {
  const arch = os.arch()
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
  const target = targetName();
  if (target === undefined) {
    return undefined;
  }
  return asserts.find(assert => assert.name === target);
}

export async function installLatestNeocmakeLsp(path: string) {
  const timeoutController = new AbortController();
  try {
    const latestRe = await Github.latestRelease(timeoutController);
    const assert = getGithubAssert(latestRe.assets);
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
