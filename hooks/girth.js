// Girth's mod.
// - Lens: the skill listing, MCP server instructions and deferred-tool list
//   Claude reads keep what this project needs; the rest collapses.
// - JIT: a collapsed server's instructions arrive on its first tool call.
// - Meter: each turn's token usage goes to the local ledger.
// - X-ray: what the last request was made of.
// - Doctor: a request that wrote the prompt cache again (after an idle gap or
//   a changed prompt) is named under the answer.
// - Keeper: while the conversation sits idle, a ping (a fork of it) reads the
//   cache before its hour runs out, so coming back doesn't write it all again.
// - Idle compaction (opt-in): a large conversation is compacted instead, while
//   its cache is still warm, and the next answer says so.
// - Subagent model (opt-in): built-in subagents run on the configured model.
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
// Doctor: the last main-loop request per model ({at, read, write}), the cache
// breaks found per turn, and the size of the last main-loop prompt.
const last = new Map()
const breaks = new Map()
let context = 0
// Keeper: what the next ping should read (the last main request's read + write).
let expected = 0
let keep = null
// Idle compaction, and the subagent settings (read once, on the first spawn).
let squeeze = null
let agentSettings = null
// A break re-wrote at least this much (and 20% of what was cached), and the
// prompt didn't shrink below 80% of it (compaction or /clear).
const BREAK_MIN = 5000

const PING_PROMPT = 'Reply with one character: .'

/** One ping: fork the conversation, record the usage; true when the cache was warm. */
export async function ping(deps, expected) {
  let r
  try {
    r = await deps.fork({ prompt: PING_PROMPT })
  } catch {
    return false
  }
  const u = r?.usage
  if (!u) return false
  const read = u.cache_read_input_tokens ?? 0
  await deps.record({
    input: u.input_tokens ?? 0,
    output: u.output_tokens ?? 0,
    read,
    write: u.cache_creation_input_tokens ?? 0,
    expected,
  })
  return read >= expected / 2
}

/** Pings one at a time per the plan; a miss or stop() ends them. */
export function keeper(deps) {
  let timer = null
  return {
    stop() {
      timer?.cancel()
      timer = null
    },
    start(plan, expected) {
      this.stop()
      let left = plan.pings
      const next = () => {
        if (left <= 0) {
          timer = null
          return
        }
        left -= 1
        timer = deps.after(plan.interval_ms, async () => {
          if (await ping(deps, expected)) next()
          else timer = null
        })
      }
      next()
    },
  }
}

/**
 * Compacts the conversation once, `plan.interval_ms` after the last turn.
 * A success is recorded and leaves a note for the next answer; a veto, a
 * refusal (headless sessions) or an error leaves nothing.
 */
export function compactor(deps) {
  let timer = null
  let generation = 0
  let note = null
  return {
    stop() {
      generation += 1
      timer?.cancel()
      timer = null
    },
    start(plan, context) {
      this.stop()
      const mine = generation
      timer = deps.after(plan.interval_ms, async () => {
        if (mine !== generation) return
        timer = null
        let r
        try {
          r = await deps.compact()
        } catch {
          return
        }
        if (!r || r.skip) return
        const before = typeof r.tokensBefore === 'number' ? r.tokensBefore : context
        note = typeof r.tokensAfter === 'number' ? { before, after: r.tokensAfter } : { before }
        if (note.after !== undefined) {
          try {
            await deps.record(note)
          } catch {
            // The note still goes under the next answer.
          }
        }
      })
    },
    takeNote() {
      const n = note
      note = null
      return n
    },
  }
}

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

  on('turn.start', async ($, e, next) => {
    if (e.agentId === undefined) {
      keep?.stop()
      squeeze?.stop()
    }
    return next(e)
  }).catch(async ($, e, next) => next(e))

  on('turn.step', async function* ($, e, next) {
    requests.set(e.turnId, (requests.get(e.turnId) ?? 0) + 1)
    if (e.agentId !== undefined) return yield* next(e)
    const start = await $.clock.now()
    const result = yield* next(e)
    try {
      const u = result?.usage
      if (u) {
        const read = u.cache_read_input_tokens ?? 0
        const write = u.cache_creation_input_tokens ?? 0
        const total = (u.input_tokens ?? 0) + read + write
        const prev = last.get(u.model)
        if (prev) {
          const expected = prev.read + prev.write
          const lost = expected - read
          if (lost >= BREAK_MIN && lost >= 0.2 * expected && total >= 0.8 * expected) {
            const found = breaks.get(e.turnId) ?? []
            found.push({ lost, rewritten: Math.min(lost, write), idle_s: Math.floor((start - prev.at) / 1000) })
            breaks.set(e.turnId, found)
          }
        }
        last.set(u.model, { at: await $.clock.now(), read, write })
        context = total
        expected = read + write
      }
    } catch {
      // Observing only: the request's result goes on unchanged.
    }
    return result
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
    const found = breaks.get(e.turnId)
    breaks.delete(e.turnId)
    if (found) turn.breaks = found
    if (e.agentId === undefined && context > 0) turn.context = context
    const note = e.agentId === undefined ? squeeze?.takeNote() : null
    if (note) turn.compacted = note
    const run = await $.process.run([launcher($), 'meter', 'record'], {
      stdin: JSON.stringify(turn),
      timeoutMs: TIMEOUT_MS,
    })
    let report = {}
    try {
      report = JSON.parse(run.stdout)
    } catch {
      return result
    }
    if (e.agentId === undefined && report.keeper?.pings > 0 && expected > 0) {
      const cwd = turn.project
      keep ??= keeper({
        after: (ms, fn) => $.clock.after(ms, fn),
        fork: (o) => $.model.fork(o),
        record: (p) =>
          $.process.run([launcher($), 'keeper', 'record'], {
            stdin: JSON.stringify({ project: cwd, model: usage.model, ...p }),
            timeoutMs: TIMEOUT_MS,
          }),
      })
      keep.start(report.keeper, expected)
    }
    if (e.agentId === undefined && report.keeper?.compact === true) {
      const cwd = turn.project
      squeeze ??= compactor({
        after: (ms, fn) => $.clock.after(ms, fn),
        compact: () => $.session.compact(),
        record: (c) =>
          $.process.run([launcher($), 'compact', 'record'], {
            stdin: JSON.stringify({ project: cwd, model: usage.model, ...c }),
            timeoutMs: TIMEOUT_MS,
          }),
      })
      squeeze.start(report.keeper, context)
    }
    if (typeof report.line === 'string' && e.agentId === undefined) {
      // next(e) resolves to the answer itself; a different text is another
      // mod's line under it, which ours follows.
      const theirs = result?.text && result.text !== e.answer ? result.text : ''
      const text = theirs ? theirs + '\n' + report.line : report.line
      return { ...result, text }
    }
    return result
  }).catch(async ($, e, next) => next(e))

  on('agent.spawn', async ($, e, next) => {
    if (e.model !== undefined || e.subagentType === 'fork' || e.provider?.plugin !== 'engine') return next(e)
    if (agentSettings === null) {
      agentSettings = {}
      const run = await $.process.run([launcher($), 'settings', '--project', await $.session.cwd()], {
        timeoutMs: TIMEOUT_MS,
      })
      if (run.exitCode === 0) agentSettings = JSON.parse(run.stdout)
    }
    const model = agentSettings?.agents?.model
    return typeof model === 'string' && model !== '' ? next({ ...e, model }) : next(e)
  }).catch(async ($, e, next) => next(e))

  on('command.run', { command: 'girth-meter' }, async ($, e) => {
    const cwd = await $.session.cwd()
    const run = await $.process.run([launcher($), 'stats', '--meter', '--project', cwd], {
      timeoutMs: TIMEOUT_MS,
    })
    return { text: run.exitCode === 0 ? run.stdout.trimEnd() : 'girth: the meter is unavailable right now' }
  }).catch(async () => ({ text: 'girth: the meter is unavailable right now' }))

  on('command.run', { command: 'girth-xray' }, async ($, e) => {
    const run = await $.process.run([launcher($), 'xray'], {
      stdin: JSON.stringify({ model, parts: [...parts.values()] }),
      timeoutMs: TIMEOUT_MS,
    })
    return { text: run.exitCode === 0 ? run.stdout.trimEnd() : 'girth: the x-ray is unavailable right now' }
  }).catch(async () => ({ text: 'girth: the x-ray is unavailable right now' }))
}
