const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {platforms} = require('./resolve.cjs');
const sums = require('./checksums.json');
for (const platform of platforms) {
    const bytes = fs.readFileSync(path.join(__dirname,'platforms',platform,'planexpo-elm'));
    if (crypto.createHash('sha256').update(bytes).digest('hex') !== sums[platform]) throw new Error(`Invalid release binary: ${platform}`);
}
