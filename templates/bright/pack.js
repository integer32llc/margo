import tar from 'tar-fs';
import { createWriteStream, mkdirSync } from 'fs';

console.log('packing template...');

const DIR = '../../src/templates';
mkdirSync(DIR, { recursive: true });

let out = createWriteStream(`${DIR}/bright.tar`);

tar.pack('./dist')
	.pipe(out)
	.on('error', (err) => console.error(err))
	.on('close', () => console.log(`packed to ${DIR}/bright.tar`));
