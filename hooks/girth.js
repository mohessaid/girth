// Girth's mod.
// - Lens: the skill listing, MCP server instructions and deferred-tool list
//   Claude reads keep what this project needs; the rest collapses.
// - JIT: a collapsed server's instructions arrive on its first tool call.
// - Meter: each turn's token usage goes to the local ledger.
// - X-ray: what the last request was made of.
// The girth binary makes every decision, and every hook fails open: on any
// error Claude Code carries on unchanged.

const TIMEOUT_MS = 5000
const WARMUP_TIMEOUT_MS = 60000

// Attachment type -> `girth lens --kind`.
const KINDS = {
  skill_listing: 'skills',
  mcp_instructions_delta: 'mcp',
  deferred_tools_delta: 'tools',
}

// type + text -> the rewrite, so a session always sends the same text and the
// prompt cache stays warm.
const rewrites = new Map()
// type -> byte counts of the rewrite now in effect, for the meter.
const lens = new Map()
// Server id -> instructions held back by Lens, and the ids already delivered.
const deferred = new Map()
const delivered = new Set()
// Requests per turn (every request re-sends the listings), and MCP servers used.
const requests = new Map()
const servers = new Set()
// X-ray: the latest size of each part of a request, and the model.
const parts = new Map()
let model = ''

function launcher($) {
  return $.plugin.root + '/skills/girth/scripts/girth'
}

function serverOf(tool) {
  const m = /^mcp__(.+?)__/.exec(tool)
  return m ? m[1] : null
}

function note(kind, name, bytes, before) {
  const part = { kind, name, bytes }
  if (before !== undefined && before !== bytes) part.before = before
  parts.set(kind + '\u0000' + name, part)
}

export function register(on) {
  on('session.start', async ($, e, next) => {
    for (const [name, description] of [
      ['girth-meter', 'Show token usage and Lens savings for this project'],
      ['girth-xray', 'Show what the last request was made of'],
    ]) {
      await $.command.register({ name, description })
    }
    // A fresh install downloads the binary once. Do it here, with room to
    // finish, instead of inside Lens's 5 s budget on the first request.
    if (!(await $.fs.exists($.plugin.root + '/skills/girth/bin/girth'))) {
      try {
        await $.process.run([launcher($), '--version'], { timeoutMs: WARMUP_TIMEOUT_MS })
      } catch {
        // Offline or slow: Lens and Meter stay off until a later session.
      }
    }
    return next(e)
  }).catch(async ($, e, next) => next(e))

  on('prompt.attachment', async ($, e, next) => {
    const kind = KINDS[e.type]
    if (kind === undefined) {
      note('attachment', e.type, e.text.length)
      return next(e)
    }
    const key = e.type + '\u0000' + e.text
    let rewrite = rewrites.get(key)
    if (rewrite === undefined) {
      const cwd = await $.session.cwd()
      const run = await $.process.run([launcher($), 'lens', '--kind', kind, '--project', cwd, '--json'], {
        stdin: e.text,
        timeoutMs: TIMEOUT_MS,
      })
      if (run.exitCode !== 0) return next(e)
      const out = JSON.parse(run.stdout)
      if (typeof out.text !== 'string') return next(e)
      rewrite = { text: out.text, before: out.bytes_before, after: out.bytes_after }
      for (const [id, text] of Object.entries(out.deferred ?? {})) deferred.set(id, text)
      rewrites.set(key, rewrite)
    }
    if (rewrite.text === e.text) lens.delete(e.type)
    else lens.set(e.type, { before: rewrite.before, after: rewrite.after })
    note('attachment', e.type, rewrite.text.length, e.text.length)
    return next({ ...e, text: rewrite.text })
  }).catch(async ($, e, next) => next(e))

  on('prompt.section', async ($, e, next) => {
    const result = await next(e)
    note('section', e.name, typeof result?.text === 'string' ? result.text.length : 0)
    return result
  }).catch(async ($, e, next) => next(e))

  on('tool.describe', async ($, e, next) => {
    const result = await next(e)
    if (typeof result?.description === 'string') note('tool', e.tool, result.description.length)
    return result
  }).catch(async ($, e, next) => next(e))

  on('tool.call', async ($, e, next) => {
    const id = serverOf(e.tool)
    if (id === null) return next(e)
    servers.add(id)
    const held = deferred.get(id)
    if (held !== undefined && !delivered.has(id)) {
      delivered.add(id)
      return {
        deny:
          id +
          ' instructions (Girth held them back until first use; read them, then call the tool again):\n\n' +
          held,
      }
    }
    return next(e)
  }).catch(async ($, e, next) => next(e))

  on('turn.step', async function* ($, e, next) {
    requests.set(e.turnId, (requests.get(e.turnId) ?? 0) + 1)
    return yield* next(e)
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    const usage = e.usage
    if (!usage) return result
    model = usage.model
    const turn = {
      project: await $.session.cwd(),
      model: usage.model,
      input: usage.input_tokens,
      output: usage.output_tokens,
      cache_read: usage.cache_read_input_tokens,
      cache_write: usage.cache_creation_input_tokens,
      agent: e.agentId !== undefined,
      aborted: e.isAborted === true,
    }
    if (lens.size > 0 && e.agentId === undefined) {
      let before = 0
      let after = 0
      for (const counts of lens.values()) {
        before += counts.before
        after += counts.after
      }
      turn.lens_before = before
      turn.lens_after = after
    }
    const sent = requests.get(e.turnId)
    requests.delete(e.turnId)
    if (sent) turn.requests = sent
    if (servers.size > 0) {
      turn.mcp = [...servers].sort()
      servers.clear()
    }
    await $.process.run([launcher($), 'meter', 'record'], {
      stdin: JSON.stringify(turn),
      timeoutMs: TIMEOUT_MS,
    })
    return result
  }).catch(async ($, e, next) => next(e))

  on('command.run', { command: 'girth-meter' }, async ($, e) => {
    const cwd = await $.session.cwd()
    const run = await $.process.run([launcher($), 'stats', '--meter', '--project', cwd], {
      timeoutMs: TIMEOUT_MS,
    })
    return { text: run.exitCode === 0 ? run.stdout.trimEnd() : 'girth: the meter is unavailable right now' }
  })

  on('command.run', { command: 'girth-xray' }, async ($, e) => {
    const run = await $.process.run([launcher($), 'xray'], {
      stdin: JSON.stringify({ model, parts: [...parts.values()] }),
      timeoutMs: TIMEOUT_MS,
    })
    return { text: run.exitCode === 0 ? run.stdout.trimEnd() : 'girth: the x-ray is unavailable right now' }
  })
}
