import { spawn } from 'node:child_process'

const args = process.argv.slice(2)
if (args.length === 0) {
  console.error('Usage: node scripts/electron-vite.mjs <dev|build|preview> [...args]')
  process.exit(1)
}

const env = { ...process.env }
delete env.ELECTRON_RUN_AS_NODE

const child = spawn('electron-vite', args, {
  stdio: 'inherit',
  env,
  shell: true
})

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal)
    return
  }
  process.exit(code ?? 1)
})

