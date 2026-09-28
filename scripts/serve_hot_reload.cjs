// Isolated real Webpack watcher for manual/browser HMR regression checks.
// Edit Main.elm in the printed temporary directory; source fixtures stay intact.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createRequire } = require('node:module');
const root = path.resolve(__dirname, '..');
const front = path.resolve(root, '../front');
const requireFront = createRequire(path.join(front, 'package.json'));
const webpack = requireFront('webpack');
const Server = requireFront('webpack-dev-server');
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'elm-rust-hmr-browser-'));
fs.mkdirSync(path.join(directory,'src'));
for (const file of ['src/Main.elm', 'elm.json']) fs.copyFileSync(path.join(root, 'tests/programs/hot-reload', file), path.join(directory, file));
fs.writeFileSync(path.join(directory,'index.html'), '<!doctype html><title>Rust Elm HMR</title><div id="app"></div><script src="/bundle.js"></script>');
fs.writeFileSync(path.join(directory,'index.js'), 'const {Elm}=require("./src/Main.elm");Elm.Main.init({node:document.getElementById("app")});');
const port = Number(process.argv[2] || 8097);
const elmCompiler = process.argv[3] || path.join(front,'scripts/elm-rust.sh');
const options = {host:'127.0.0.1',port,hot:true,contentBase:directory,publicPath:'/',stats:'errors-only',overlay:false};
const config = {mode:'development',context:directory,entry:path.join(directory,'index.js'),
 output:{path:path.join(directory,'dist'),filename:'bundle.js',publicPath:'/'},
 resolveLoader:{modules:[path.join(front,'node_modules')]},
 module:{rules:[{test:/\.elm$/,use:[{loader:'elm-hot-webpack-loader'},{loader:'elm-webpack-loader',options:{cwd:directory,forceWatch:true,pathToElm:elmCompiler}}]}]},
 plugins:[new webpack.HotModuleReplacementPlugin()],devtool:false};
Server.addDevServerEntrypoints(config, options);
const compiler=webpack(config);
compiler.hooks.done.tap('report',stats=>console.log(JSON.stringify({hash:stats.hash,errors:stats.hasErrors(),warnings:stats.hasWarnings()})));
const server=new Server(compiler,options);
server.listen(port,'127.0.0.1',error=>{if(error)throw error;console.log(JSON.stringify({url:`http://127.0.0.1:${port}`,directory}));});
for (const signal of ['SIGTERM','SIGINT']) process.on(signal,()=>server.close(()=>process.exit(0)));
