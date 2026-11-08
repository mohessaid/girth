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
// Outcomes: each loop's Bash calls this turn ('' is the main loop), for the
// binary to classify. Only its labels reach the ledger.
const bash = new Map()
// Estimated tokens of large tool results this turn, per loop (agentId; '' is the main
// loop), then per tool.
const large = new Map()
const OUTPUT_TAIL = 4000
const MAX_CALLS = 64
// A tool result this long (2,000 tokens at 4 characters each) is a large one. Lengths
// are UTF-16 units; the importers count characters, the same for ASCII.
// `LARGE_OUTPUT_CHARS` in src/ledger.rs keeps the same figure for imports.
const LARGE_OUTPUT_CHARS = 8000
// A break re-wrote at least this much (and 20% of what was cached), and the
// prompt didn't shrink below 80% of it (compaction or /clear).
const BREAK_MIN = 5000

// `girth advise --brief`: one line under the session's first main answer, at most.
let advised = false
// Claude Code's requests outside turns (query_source → totals), from its collector
// records; flushed to `girth side record` after a main-loop turn and at session end.
const side = new Map()
// The main loop's requests since its last turn: Claude Code's own price for them,
// kept for the turn when their tokens add up to its usage.
let main = { requests: 0, tokens: 0, micros: 0, priced: true }
// Sources already counted through turn.complete, and mod forks (Girth's own pings).
const TURN_SOURCES = new Set(['repl_main_thread', 'sdk', 'hook_prompt', 'plugin_model_complete', 'plugin_model_classify'])

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

// Adds a large result's estimated tokens to its loop's turn.
function measure(agentId, tool, text) {
  if (text.length < LARGE_OUTPUT_CHARS) return
  const key = agentId ?? ''
  const tools = large.get(key) ?? {}
  tools[tool] = (tools[tool] ?? 0) + Math.ceil(text.length / 4)
  large.set(key, tools)
}

const count = (v) => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : 0)

// Add one api_request record's numbers to its source's totals. Only numbers, the
// model id and the source name are kept: never user, organization or request ids.
function tally(a) {
  const source = a?.query_source
  const model = a?.model
  if (typeof source !== 'string' || typeof model !== 'string' || model === '') return
  if (source === 'repl_main_thread' || source === 'sdk') {
    main.requests += 1
    main.tokens += count(a.input_tokens) + count(a.output_tokens) + count(a.cache_read_tokens) + count(a.cache_creation_tokens)
    if (typeof a.cost_usd_micros === 'number' && Number.isFinite(a.cost_usd_micros)) main.micros += a.cost_usd_micros
    else main.priced = false
    return
  }
  if (TURN_SOURCES.has(source) || source.startsWith('agent:')) return
  const key = source + '\u0000' + model
  const t = side.get(key) ?? { source, model, requests: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, cost_micros: 0 }
  t.requests += 1
  t.input += count(a.input_tokens)
  t.output += count(a.output_tokens)
  t.cache_read += count(a.cache_read_tokens)
  t.cache_write += count(a.cache_creation_tokens)
  // One unpriced request leaves the whole total to Girth's list prices.
  t.cost_micros = t.cost_micros !== undefined && typeof a.cost_usd_micros === 'number' ? t.cost_micros + a.cost_usd_micros : undefined
  side.set(key, t)
}

async function flushSide($, session, timeoutMs = TIMEOUT_MS) {
  if (side.size === 0) return
  const project = await $.session.cwd()
  const rows = [...side.values()].map((t) => ({ project, ...(session ? { session } : {}), ...t }))
  side.clear()
  try {
    await $.process.run([launcher($), 'side', 'record'], { stdin: JSON.stringify(rows), timeoutMs })
  } catch {
    // Metering only: a missing binary costs these totals, nothing else.
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

// The last OUTPUT_TAIL characters, never starting inside a surrogate pair:
// half a character would make the meter's JSON invalid.
function tail(text) {
  const cut = text.slice(-OUTPUT_TAIL)
  const first = cut.charCodeAt(0)
  return first >= 0xdc00 && first <= 0xdfff ? cut.slice(1) : cut
}

// The session id lets `girth advise` follow a skill and a session's age; never fatal.
async function sessionId($) {
  try {
    const id = await $.session.id()
    return typeof id === 'string' && id !== '' ? id : undefined
  } catch {
    return undefined
  }
}

export function register(on) {
  on('session.start', async ($, e, next) => {
    advised = false
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
    if (e.tool === 'Bash') {
      const result = await next(e)
      try {
        const calls = bash.get(e.agentId ?? '') ?? []
        // A denied call never ran. Keep the last MAX_CALLS: a turn's commit
        // usually comes at its end.
        if (result?.deny === undefined) {
          if (calls.length >= MAX_CALLS) calls.shift()
          const r = result?.result
          const output =
            typeof result?.text === 'string'
              ? result.text
              : [r?.stdout, r?.stderr].filter((t) => typeof t === 'string' && t !== '').join('\n')
          measure(e.agentId, 'Bash', output)
          calls.push({
            command: String(e.command),
            output: tail(output),
            error: result?.isError === true,
            background: Boolean(r?.backgroundTaskId || r?.interrupted),
          })
          bash.set(e.agentId ?? '', calls)
        }
      } catch {
        // Observing only: the call's result goes back unchanged.
      }
      return result
    }
    const id = serverOf(e.tool)
    if (id !== null) {
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
    }
    const result = await next(e)
    try {
      // `text` is what the model reads; an image Read's is empty.
      if (result?.deny === undefined && typeof result?.text === 'string') measure(e.agentId, e.tool, result.text)
    } catch {
      // Observing only: the result goes back unchanged.
    }
    return result
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

  on('telemetry.log', { to: 'collector' }, async ($, e, next) => {
    try {
      if (e.event === 'api_request') tally(e.attributes)
    } catch {
      // Observing only: the record goes on unchanged.
    }
    return next(e)
  }).catch(async ($, e, next) => next(e))

  on('session.end', async ($, e, next) => {
    try {
      const left = next.budget?.remainingMs
      const budget = typeof left === 'number' ? Math.max(100, Math.min(TIMEOUT_MS, left - 50)) : TIMEOUT_MS
      await flushSide($, e.sessionId || undefined, budget)
    } catch {
      // An exit is never held up by metering.
    }
    return next(e)
  }).catch(async ($, e, next) => next(e))

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId === undefined) await flushSide($, await sessionId($))
    const calls = bash.get(e.agentId ?? '')
    bash.delete(e.agentId ?? '')
    const big = large.get(e.agentId ?? '')
    large.delete(e.agentId ?? '')
    const usage = e.usage
    if (!usage) return result
    model = usage.model
    const turn = {
      project: await $.session.cwd(),
      session: await sessionId($),
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
    if (calls) turn.bash = calls
    if (big) turn.tool_output = big
    const found = breaks.get(e.turnId)
    breaks.delete(e.turnId)
    if (found) turn.breaks = found
    if (e.agentId === undefined && context > 0) turn.context = context
    if (e.agentId === undefined) {
      const sum = (turn.input ?? 0) + (turn.output ?? 0) + (turn.cache_read ?? 0) + (turn.cache_write ?? 0)
      if (main.requests > 0 && main.priced && main.tokens === sum) turn.cost_micros = Math.round(main.micros)
      main = { requests: 0, tokens: 0, micros: 0, priced: true }
    }
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
    if (e.agentId !== undefined) return result
    const lines = typeof report.line === 'string' ? [report.line] : []
    if (!advised) {
      advised = true
      try {
        const brief = await $.process.run([launcher($), 'advise', '--brief'], { timeoutMs: TIMEOUT_MS })
        const line = brief.exitCode === 0 ? brief.stdout.trim() : ''
        // One line of advice or nothing: anything else (a crash, an old binary) is not shown.
        if (line.startsWith('girth: ') && !line.includes('\n')) lines.push(line)
      } catch {
        // Advice is never worth breaking a turn over.
      }
    }
    if (lines.length === 0) return result
    // next(e) resolves to the answer itself; a different text is another
    // mod's line under it, which ours follows.
    const theirs = result?.text && result.text !== e.answer ? result.text : ''
    const text = [theirs, ...lines].filter((t) => t !== '').join('\n')
    return { ...result, text }
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
