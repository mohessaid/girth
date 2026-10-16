// Renders the scene frame by frame in headless Chrome and encodes it with ffmpeg.
//   node render.mjs <style> <seconds> <out.mp4> [width height]
import { createServer } from 'node:http'
import { readFile, mkdir, rm, writeFile } from 'node:fs/promises'
import { extname, join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'
import { chromium } from 'playwright-core'

const [style = 'ink', seconds = '5', out = `pilot-${style}.mp4`, w = '1920', h = '1080'] = process.argv.slice(2)
const root = dirname(fileURLToPath(import.meta.url))
const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript' }
const server = createServer(async (req, res) => {
  try {
    const path = join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname))
    if (!path.startsWith(root)) throw new Error('outside')
    const body = await readFile(path)
    res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' })
    res.end(body)
  } catch {
    res.writeHead(404).end()
  }
}).listen(0)
const port = server.address().port

const exe = process.env.CHROME ?? `${process.env.HOME}/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome`
const browser = await chromium.launch({ executablePath: exe, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] })
const page = await browser.newPage({ viewport: { width: +w, height: +h } })
page.on('console', (m) => console.log('page:', m.text()))
page.on('pageerror', (e) => console.error('page error:', e.message))
await page.goto(`http://127.0.0.1:${port}/index.html?style=${style}&w=${w}&h=${h}`)
await page.waitForFunction('window.ready === true', null, { timeout: 60000 })

const frames = join(root, 'frames', style)
await rm(frames, { recursive: true, force: true })
await mkdir(frames, { recursive: true })
const total = Math.round(+seconds * 30)
const stills = process.env.FRAMES?.split(',').map(Number)
// Stills go to frames/<style>/ for review; a full render streams into ffmpeg.
const ffmpeg = stills ? null : spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', '30',
  '-i', '-', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18', '-preset', 'slow', '-movflags', '+faststart', '-an', out],
  { stdio: ['pipe', 'inherit', 'inherit'] })
const done = ffmpeg && new Promise((ok, fail) => ffmpeg.on('close', (c) => (c === 0 ? ok() : fail(new Error('ffmpeg ' + c)))))
for (const n of stills ?? [...Array(total).keys()]) {
  const data = await page.evaluate((n) => {
    window.renderFrame(n)
    return document.querySelector('canvas').toDataURL('image/png')
  }, n)
  const png = Buffer.from(data.split(',')[1], 'base64')
  if (stills) await writeFile(join(frames, `${String(n).padStart(5, '0')}.png`), png)
  else if (!ffmpeg.stdin.write(png)) await new Promise((r) => ffmpeg.stdin.once('drain', r))
  if (n % 150 === 0) process.stdout.write(`${style} ${n}/${total}\n`)
}
await browser.close()
server.close()
if (ffmpeg) {
  ffmpeg.stdin.end()
  await done
  console.log('wrote', out)
}
