// Runs before Theia creates the Electron backend. Keep built-ins available in packaged apps.
const path = require('node:path');
const { app } = require('electron');
// Select the documented profile before Theia obtains Electron's single-instance lock.
const profileIndex = process.argv.indexOf('--electronUserData');
const profileInline = process.argv.find(arg => arg.startsWith('--electronUserData='));
const profile = profileInline ? profileInline.slice('--electronUserData='.length) : profileIndex >= 0 ? process.argv[profileIndex + 1] : undefined;
if (profile && !profile.startsWith('--')) {
  const directory = path.resolve(profile);
  require('node:fs').mkdirSync(directory, { recursive: true });
  app.setPath('userData', directory);
}
const plugins = app.isPackaged ? path.join(process.resourcesPath, 'plugins') : path.resolve(__dirname, '../../plugins');
if (!process.argv.some(arg => arg.startsWith('--plugins'))) process.argv.push('--plugins=local-dir:' + plugins);
if (!process.argv.some(arg => arg.startsWith('--hostname'))) process.argv.push('--hostname=127.0.0.1');
require('./lib/backend/electron-main.js');
