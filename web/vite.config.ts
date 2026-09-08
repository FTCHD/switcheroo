import path from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// In development the page is served by Vite and /api is proxied to a `switcheroo serve --dev`
// instance (fixed session token "dev"). In production the Rust server embeds `dist/`.
export default defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: { alias: { '@': path.resolve(__dirname, './src') } },
    server: {
        port: 5173,
        strictPort: true,
        proxy: {
            '/api': {
                target: process.env.SWITCHEROO_API ?? 'http://127.0.0.1:7777',
                changeOrigin: false,
            },
        },
    },
    build: { outDir: 'dist', emptyOutDir: true, sourcemap: false },
})
