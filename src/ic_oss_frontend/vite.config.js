import { sveltekit } from '@sveltejs/kit/vite'
import dotenv from 'dotenv'
import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import environment from 'vite-plugin-environment'

dotenv.config({ path: '../../.env' })

if (process.env.PUBLIC_DFX_NETWORK === 'ic') {
  process.env.NODE_ENV === 'production'
}

export default defineConfig({
  define: {
    'process.env.NODE_ENV':
      process.env.NODE_ENV === 'production' ? '"production"' : '"development"'
  },
  build: {
    emptyOutDir: true
  },
  optimizeDeps: {
    esbuildOptions: {
      define: {
        global: 'globalThis'
      }
    }
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:4943',
        changeOrigin: true
      }
    }
  },
  plugins: [
    sveltekit(),
    environment('all', { prefix: 'CANISTER_' }),
    environment('all', { prefix: 'DFX_' }),
  ],
  test: {},
  resolve: {
    alias: {
      $declarations: resolve('./src/declarations')
    }
  }
})
