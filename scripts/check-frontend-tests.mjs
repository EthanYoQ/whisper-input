import { spawnSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

function testFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? testFiles(path) : /\.test\.tsx?$/.test(entry.name) ? [path] : [];
  });
}

const files = testFiles('src').sort();
if (!files.length) throw new Error('No frontend tests found');
console.log(`Running ${files.length} frontend test files`);
const result = spawnSync(process.execPath, [resolve('node_modules/tsx/dist/cli.mjs'), '--test', ...files], {
  stdio: 'inherit',
  shell: false,
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
