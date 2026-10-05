import { createReadStream, existsSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";

// Dev server only: /dev/<file> serves the local game files for automated
// tests (`?devfiles`). Set DUNE_DIR or default to ../../Cryogenic/dune.
function devFiles(): Plugin {
  const dir = process.env.DUNE_DIR ?? resolve(__dirname, "../../Cryogenic/dune");
  return {
    name: "dev-game-files",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/dev/", (req, res) => {
        const name = (req.url ?? "").replace(/^\//, "");
        const path = resolve(dir, name);
        if (!/^[A-Z0-9_.]+$/i.test(name) || !existsSync(path)) {
          res.statusCode = 404;
          res.end();
          return;
        }
        res.setHeader("Content-Length", statSync(path).size);
        createReadStream(path).pipe(res);
      });
    },
  };
}

// Dev server only: /dev-soundtrack.zip serves a locally purchased soundtrack
// ZIP for automated tests (`?devmusic`). Set DUNE_SOUNDTRACK to its path.
function devSoundtrack(): Plugin {
  return {
    name: "dev-soundtrack",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/dev-soundtrack.zip", (_req, res) => {
        const path = process.env.DUNE_SOUNDTRACK;
        if (!path || !existsSync(path)) {
          res.statusCode = 404;
          res.end("set DUNE_SOUNDTRACK");
          return;
        }
        res.setHeader("Content-Length", statSync(path).size);
        createReadStream(path).pipe(res);
      });
    },
  };
}

// Dev server only: /dev-hdpack.zip serves a locally generated HD art pack
// for automated tests (`?devpack`). Set DUNE_HDPACK or default to
// ../../dune-hd-pack.zip (outside the repository: packs are never committed).
function devHdPack(): Plugin {
  return {
    name: "dev-hdpack",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/dev-hdpack.zip", (_req, res) => {
        const path = process.env.DUNE_HDPACK ?? resolve(__dirname, "../../dune-hd-pack.zip");
        if (!existsSync(path)) {
          res.statusCode = 404;
          res.end("set DUNE_HDPACK");
          return;
        }
        res.setHeader("Content-Length", statSync(path).size);
        createReadStream(path).pipe(res);
      });
    },
  };
}

export default defineConfig({
  base: "./",
  plugins: [devFiles(), devSoundtrack(), devHdPack()],
  server: { port: 5174 },
  preview: { port: 4174 },
  worker: { format: "es" },
  build: { target: "es2022" },
});
