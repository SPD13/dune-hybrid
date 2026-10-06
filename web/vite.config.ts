import { spawn, type ChildProcess } from "node:child_process";
import { createReadStream, createWriteStream, existsSync, mkdirSync, statSync, type WriteStream } from "node:fs";
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

// Dev server only: the trailer recorder (`?devtrailer`, src/dev/trailer.ts)
// posts raw RGBA frames and 48 kHz stereo f32 audio here; ffmpeg encodes them
// into out/trailer/trailer.mp4 (outside git: it shows the game's art).
function devTrailer(): Plugin {
  const dir = resolve(__dirname, "../out/trailer");
  let ffmpeg: ChildProcess | null = null;
  let audio: WriteStream | null = null;
  const run = (args: string[]) =>
    new Promise<number>((done) => spawn("ffmpeg", args, { stdio: ["ignore", "inherit", "inherit"] }).on("exit", (code) => done(code ?? 1)));
  return {
    name: "dev-trailer",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/dev-trailer/", (req, res) => {
        const url = new URL(req.url ?? "/", "http://x");
        const reply = (body: object) => {
          res.setHeader("Content-Type", "application/json");
          res.end(JSON.stringify(body));
        };
        switch (url.pathname) {
          case "/start": {
            mkdirSync(dir, { recursive: true });
            const [w, h, fps] = ["w", "h", "fps"].map((k) => url.searchParams.get(k) ?? "");
            // BT.709 throughout, so players show the colours the browser drew.
            ffmpeg = spawn(
              "ffmpeg",
              ["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", `${w}x${h}`, "-r", fps, "-i", "-"]
                .concat(["-vf", "scale=out_color_matrix=bt709:out_range=tv", "-c:v", "libx264", "-preset", "slow", "-crf", "16", "-tune", "animation"])
                .concat(["-pix_fmt", "yuv420p", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", `${dir}/video.mp4`]),
              { stdio: ["pipe", "inherit", "inherit"] },
            );
            audio = createWriteStream(`${dir}/audio.f32`);
            reply({ ok: true });
            return;
          }
          case "/frame":
          case "/audio": {
            const sink = url.pathname === "/frame" ? ffmpeg?.stdin : audio;
            if (!sink) {
              res.statusCode = 409;
              res.end();
              return;
            }
            req.pipe(sink, { end: false });
            req.on("end", () => reply({ ok: true }));
            return;
          }
          case "/still": {
            mkdirSync(dir, { recursive: true });
            const name = (url.searchParams.get("name") ?? "still").replace(/[^\w-]/g, "");
            req.pipe(createWriteStream(`${dir}/${name}.png`)).on("finish", () => reply({ ok: true }));
            return;
          }
          case "/finish": {
            const seconds = Number(url.searchParams.get("seconds"));
            const ff = ffmpeg;
            ffmpeg = null;
            audio?.end();
            audio = null;
            if (!ff) {
              res.statusCode = 409;
              res.end();
              return;
            }
            ff.on("exit", async () => {
              const fade = Math.max(0, seconds - 2.5).toFixed(2);
              const code = await run(
                ["-y", "-loglevel", "error", "-i", `${dir}/video.mp4`, "-f", "f32le", "-ar", "48000", "-ac", "2", "-i", `${dir}/audio.f32`]
                  .concat(["-af", `loudnorm=I=-16:TP=-1.5:LRA=11,afade=t=out:st=${fade}:d=2.5`, "-c:v", "copy", "-c:a", "aac", "-b:a", "192k"])
                  .concat(["-ar", "48000", "-shortest", "-movflags", "+faststart", `${dir}/trailer.mp4`]),
              );
              reply({ ok: code === 0, path: `${dir}/trailer.mp4` });
            });
            ff.stdin?.end();
            return;
          }
        }
        res.statusCode = 404;
        res.end();
      });
    },
  };
}

export default defineConfig({
  base: "./",
  plugins: [devFiles(), devSoundtrack(), devHdPack(), devTrailer()],
  server: { port: 5174 },
  preview: { port: 4174 },
  worker: { format: "es" },
  build: { target: "es2022" },
});
