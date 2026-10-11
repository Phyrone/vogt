import { defineConfig } from 'drizzle-kit';
import { homedir } from 'node:os';
import { join } from 'node:path';

// where Tauri's app_config_dir puts the app's files; tauri-plugin-sql keeps the database there
const configDir =
	process.platform === 'darwin'
		? join(homedir(), 'Library', 'Application Support')
		: process.platform === 'win32'
			? (process.env.APPDATA ?? join(homedir(), 'AppData', 'Roaming'))
			: (process.env.XDG_CONFIG_HOME ?? join(homedir(), '.config'));

export default defineConfig({
	schema: './src/lib/db/schema.ts',
	dialect: 'sqlite',
	// the identifier in src-tauri/tauri.conf.json and the name in src/lib/db/index.ts
	dbCredentials: { url: `file:${join(configDir, 'de.phyrone.vogt', 'vogt.db')}` },
	verbose: true,
	strict: true
});
