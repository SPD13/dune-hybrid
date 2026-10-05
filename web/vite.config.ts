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

export default defineConfig({
  base: "./",
  plugins: [devFiles()],
  worker: { format: "es" },
  build: { target: "es2022" },
});
