/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import { workspace, ExtensionContext } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from 'vscode-languageclient/node';
import { installLatestNeocmakeLsp } from './download';

let client: LanguageClient;

export async function activate(context: ExtensionContext) {
  const config = workspace.getConfiguration("neocmakelsp");
  const allAsJson = JSON.parse(JSON.stringify(config));

  let neocmakelspExecutable = undefined;

  if (allAsJson.tcp === true) {
    neocmakelspExecutable = {
      command: "nc",
      args: ['localhost', '9257']
    }
  } else {
    let realPath = "neocmakelsp";
    if (allAsJson.localtarget !== true) {
      const exPath = context.extensionPath;

      let path = await installLatestNeocmakeLsp(exPath);
      if (path !== undefined) {
        realPath = path;
      }
    }
    // The server is implemented in node
    // If the extension is launched in debug mode then the debug server options are used
    // Otherwise the run options are used
    neocmakelspExecutable = {
      command: realPath,
      args: ['--stdio'],
    };
  }
  const serverOptions: ServerOptions = {
    run: neocmakelspExecutable,
    debug: neocmakelspExecutable
  };

  // Options to control the language client
  const clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents
    documentSelector: [{ scheme: 'file', language: 'cmake' }],
    synchronize: {
      // Notify the server about file changes to '.clientrc files contained in the workspace
      fileEvents: workspace.createFileSystemWatcher('**/CMakeCache.txt')
    },
    initializationOptions: {
      semantic_token: true
    }
  };

  // Create the language client and start the client.
  client = new LanguageClient(
    'neocmakelsp',
    'neocmakelsp',
    serverOptions,
    clientOptions
  );

  // Start the client. This will also launch the server
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
