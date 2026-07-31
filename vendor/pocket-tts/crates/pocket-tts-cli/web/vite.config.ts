import path from "path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  base: "./",
  plugins: [tailwindcss(), react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      '/stream': 'http://localhost:3000',
      '/generate': 'http://localhost:3000',
      '/health': 'http://localhost:3000',
      '/wasm': 'http://localhost:3000',
    }
  }
})
