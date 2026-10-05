/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        surface: {
          base: "#0F172A",
          panel: "#1E293B",
          card: "#334155",
          hover: "#475569",
          border: "#334155",
        },
        accent: {
          cyan: "#38BDF8",
          blue: "#3B82F6",
          green: "#4ADE80",
          yellow: "#FACC15",
          red: "#F87171",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "SF Mono", "Menlo", "Consolas", "monospace"],
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
      },
    },
  },
  plugins: [],
};
