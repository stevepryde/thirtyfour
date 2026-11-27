import { defineConfig, markdown, rustdoc } from "sourcey";

export default defineConfig({
  name: "thirtyfour",
  repo: "https://github.com/stevepryde/thirtyfour",
  siteUrl: "https://stevepryde.github.io",
  baseUrl: "/thirtyfour/api",
  editBranch: "main",
  theme: {
    preset: "api-first",
    colors: { primary: "#2563eb" },
  },
  navigation: {
    tabs: [
      {
        tab: "Documentation",
        slug: "",
        groups: [
          {
            group: "Getting Started",
            pages: ["index", "quickstart"],
          },
        ],
      },
      {
        tab: "Rust API",
        slug: "rust-api",
        source: rustdoc({
          manifest: "../../thirtyfour/Cargo.toml",
          crates: ["thirtyfour", "thirtyfour-macros"],
          mode: "live",
          toolchain: "nightly-2026-05-15",
          doctestsIndex: true,
        }),
      },
    ],
  },
  navbar: {
    links: [
      { type: "github", href: "https://github.com/stevepryde/thirtyfour" },
    ],
  },
  footer: {
    links: [
      { type: "github", href: "https://github.com/stevepryde/thirtyfour" },
    ],
  },
});
