import adapter from "@sveltejs/adapter-static";

/** @type {import('@sveltejs/kit').Config} */
const outDir = process.env.UI_OUT_DIR ?? "build";

const config = {
  compilerOptions: {
    runes: ({ filename }) =>
      filename.split(/[/\\]/).includes("node_modules") ? undefined : true,
  },
  kit: {
    adapter: adapter({
      pages: outDir,
      assets: outDir,
      fallback: "index.html",
      precompress: false,
      strict: false,
    }),
    alias: {
      $components: "src/components",
      $lib: "src/lib",
      $stores: "src/stores",
      $utils: "src/utils",
    },
  },
};

export default config;
