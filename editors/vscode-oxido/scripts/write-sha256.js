const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const files = process.argv.slice(2);
if (files.length === 0) {
  throw new Error('indica al menos un archivo para calcular su SHA-256');
}

for (const file of files) {
  const hash = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const name = path.basename(file);
  fs.writeFileSync(`${file}.sha256`, `${hash}  ${name}\n`);
  process.stdout.write(`${hash}  ${name}\n`);
}
