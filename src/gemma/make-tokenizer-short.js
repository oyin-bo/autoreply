const fs = require('fs');
const tokenizer = require('./tokenizer.json');
fs.writeFileSync(
  './tokenizer-short.json',
  '{\n' +
    Object.entries(tokenizer)
      .map(([key, value]) => `  "${key}": ${JSON.stringify(value)}`).join(',\n') + '\n}',
  'utf8'
);