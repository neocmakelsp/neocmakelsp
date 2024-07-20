import which from 'which'
import * as child_process from 'child_process'

const githubReleaseURL = 'https://api.github.com/repos/Decodetalkers/neocmakelsp/releases/latest';

export interface Release {
  name: string, tag_name: string, assets: Array<Asset>,
}

export interface Asset {
  name: string, browser_download_url: string,
}

export async function isLatestRelease(path: string, abort: AbortController) {
  const latestversion = await latestRelease(abort);
  const version = await getNeocmakeVersion(path);
  return latestversion.tag_name.substring(1) === version
}

export async function latestRelease(timeoutController: AbortController) {
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

