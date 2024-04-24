/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/html.rs"],
  theme: {
    extend: {
      colors: {
        theme: {
          orange: "#f8b595",
          salmon: {
            light: "#f1bbbf",
            DEFAULT: "#f67280",
          },
          rose: {
            light: "#d6a1a8",
            DEFAULT: "#c06c84",
          },
          purple: "#6c5b7c",
        },
      },
    },
  },
  plugins: [],
};
