import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// In dev, Vite serves the UI and proxies /api to `spin up` (default :3000).
// In production, Spin serves both: the fileserver at / and the Rust API at /api.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { '/api': 'http://127.0.0.1:3000' },
  },
})
