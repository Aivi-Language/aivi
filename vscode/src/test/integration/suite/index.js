/* eslint-disable no-console */
const path = require('node:path');
const Mocha = require('mocha');

function run() {
  const mocha = new Mocha({
    ui: 'tdd',
    timeout: 60_000,
    color: true,
  });

  mocha.addFile(path.resolve(__dirname, './lsp-completions.test.js'));

  return new Promise((resolve, reject) => {
    try {
      mocha.run((failures) => {
        if (failures > 0) {
          reject(new Error(`${failures} integration test(s) failed.`));
        } else {
          resolve();
        }
      });
    } catch (err) {
      reject(err);
    }
  });
}

module.exports = { run };

