const fs = require('node:fs');

fs.mkdirSync('dist', { recursive: true });
fs.rmSync('server', { recursive: true, force: true });
