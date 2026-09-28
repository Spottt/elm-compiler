# `--make-worker`: a persistent compiler process

`elm make` starts from scratch on every run: it reads `elm.json`, the package
interfaces and every module's cached interface again. For a watch loop (a
bundler plugin, a dev server, an editor), `--make-worker` keeps one compiler
process alive and answers `make` requests over stdin/stdout. Between requests
it keeps the parsed project and the analysis of the unchanged part of the
module graph in memory, so a rebuild after an edit only redoes what depends on
the edited module.

On our 677-module application, a rebuild after a value edit takes 0.82 s with
the worker versus 1.75 s with `elm make --incremental`, at the cost of more
memory (the process keeps what it analysed).

The worker is only the compiler side. Bundler integration (for example a
Webpack or Vite plugin that sends requests on file changes, or hot reload that
re-sends only changed JavaScript) is not part of this repository yet.

## Starting it

```sh
cd path/to/project   # the directory with elm.json, or below it
elm --make-worker
```

The working directory and environment are fixed for the life of the process.
It exits when stdin is closed. Run one worker per project; requests are handled
one at a time, in order.

## Protocol

One JSON object per line in each direction (UTF-8, `\n`-terminated). Every
response carries `"protocol": 1`.

On start, before reading anything, the worker writes:

```json
{"protocol":1,"ready":true,"capabilities":{"batch":true,"streaming_batch":true},"executable":"/abs/path/to/binary"}
```

### Single request

```json
{"arguments":["src/Main.elm","--output=main.js"],"changed":["src/Page/Home.elm"],"dependencies":"/tmp/deps.json"}
```

| Field          | Required | Meaning |
| -------------- | -------- | ------- |
| `arguments`    | yes      | Exactly what you would pass after `elm make` (entries, `--output`, `--debug`, `--optimize`…). `--report=json` is implied. |
| `changed`      | no       | Paths you know changed since the previous request. Only a hint to find the reusable prefix faster; correctness never depends on it (sources are re-checked). |
| `dependencies` | no       | A file the worker writes after a successful analysis: `{"version":1,"files":[...]}`, the absolute paths of every `elm.json` and module the build used. Useful to set up file watching. |

Response:

```json
{"protocol":1,"ok":true,"error":null,"report":null,"cache":{...},"analysis":{...}}
```

On failure, `ok` is `false`, `error` holds the diagnostic as the terminal would
print it, and `report` holds the same diagnostic in the `--report=json`
format. `cache` and `analysis` are counters for diagnostics and benchmarks;
their fields may change between versions.

### Batch

Several entry points (for example the apps of one project) can share one
analysis:

```json
{"batch":[{"arguments":["src/A.elm","--output=a.js"]},{"arguments":["src/B.elm","--output=b.js"]}]}
```

The response has `results`, an array of single-request responses in the same
order. A batch holds 1 to 32 requests. With `"stream": true` next to `batch`,
the worker writes one line per request as soon as it finishes
(`{"protocol":1,"index":0,"result":{...}}`) and then
`{"protocol":1,"batch_done":true,"count":2}`.

## Example (Node.js)

```js
const { spawn } = require('node:child_process');
const readline = require('node:readline');

const worker = spawn('npx', ['elm', '--make-worker'], { cwd: projectDir, stdio: ['pipe', 'pipe', 'inherit'] });
const lines = readline.createInterface({ input: worker.stdout })[Symbol.asyncIterator]();

async function next() { return JSON.parse((await lines.next()).value); }

await next(); // {"ready":true,...}
async function make(args, changed = []) {
  worker.stdin.write(JSON.stringify({ arguments: args, changed }) + '\n');
  return next();
}

const result = await make(['src/Main.elm', '--output=main.js'], ['src/Main.elm']);
if (!result.ok) console.error(result.error);
```

## Stability

The protocol is versioned by the `protocol` field. The request fields and the
`ok`/`error`/`report` response fields above are the supported surface; other
response fields are informational. The former name `--internal-make-worker` is
still accepted.
