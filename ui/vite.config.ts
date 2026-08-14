import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: './',  // Relative paths para rust-embed SPA fallback
  server: {
    port: 5173,
    open: true,
    proxy: {
      '/api': 'http://localhost:8080',
    },
  },
  build: {
    assetsInlineLimit: 4096,
  },
})
