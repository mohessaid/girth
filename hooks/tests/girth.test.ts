import { expect, test } from 'claude-code/testing'

const LISTING = 'Skills:\n\n- girth:girth: Use when.\n- a: A.\n- b: B.\n'
const REWRITTEN = 'Skills:\n\n- girth:girth: Use when.\n- Also installed (names only): a, b\n'

type Run = { argv: string[]; stdin?: string }

function lensOut() {
  return JSON.stringify({ text: REWRITTEN, bytes_before: LISTING.length, bytes_after: REWRITTEN.length, described: [], collapsed: [] })
}

const USAGE = {
  input_tokens: 2,
  output_tokens: 7,
  cache_read_input_tokens: 8605,
  cache_creation_input_tokens: 26631,
  model: 'claude-opus-5-5',
}

test('the skill listing is rewritten by girth lens, once per listing', async ($, on) => {
  const runs: Run[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    runs.push({ argv: e.argv, stdin: e.init?.stdin })
    return { value: { exitCode: 0, stdout: lensOut(), stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  const first = await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  const second = await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  expect(first.text).toBe(REWRITTEN)
  expect(second.text).toBe(REWRITTEN)
  expect(runs.length).toBe(1)
  expect(runs[0].argv.slice(1)).toEqual(['lens', '--kind', 'skills', '--project', '/work/app', '--json'])
  expect(runs[0].argv[0]).toMatch(/\/skills\/girth\/scripts\/girth$/)
  expect(runs[0].stdin).toBe(LISTING)
})

test('other attachments pass through untouched', async ($, on) => {
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  const out = await $.prompt.attachment({ type: 'todo_reminder', text: 'remember', origin: 'engine' })
  expect(out.text).toBe('remember')
})

test('a failing binary leaves the listing unchanged', async ($, on) => {
  on('session.cwd', () => ({ value: '/work/app' }))
  // Even output that parses is ignored when the binary reports failure.
  on('process.run', () => ({ value: { exitCode: 2, stdout: lensOut(), stderr: 'failed' } }))
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  const out = await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  expect(out.text).toBe(LISTING)
})

test('a binary that cannot run or prints garbage leaves the listing unchanged', async ($, on) => {
  let calls = 0
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => {
    calls += 1
    return calls === 1 ? { deny: 'timed out' } : { value: { exitCode: 0, stdout: 'not json', stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  const a = await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  const b = await $.prompt.attachment({ type: 'skill_listing', text: LISTING + '- c: C.\n', origin: 'engine' })
  expect(a.text).toBe(LISTING)
  expect(b.text).toBe(LISTING + '- c: C.\n')
})

test('turn.complete records usage, with the Lens byte counts on main-loop turns', async ($, on) => {
  const runs: Run[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    runs.push({ argv: e.argv, stdin: e.init?.stdin })
    return { value: { exitCode: 0, stdout: e.argv[1] === 'lens' ? lensOut() : '', stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  on('turn.complete', () => ({ text: '' }))
  on('turn.step', async function* ($, e) {
    return { turnId: e.turnId, index: e.index, answer: 'ok', toolUses: [], stopReason: 'end_turn', usage: null }
  })
  await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  for (const index of [0, 1]) {
    const stream = $.turn.step({ turnId: 't1', index, model: 'claude-test', messageCount: 1 })
    let step = await stream.next()
    while (step.done !== true) step = await stream.next()
  }
  await $.turn.complete({ turnId: 't1', answer: 'hi', durationMs: 5, isAborted: false, usage: USAGE })
  await $.turn.complete({ turnId: 't2', answer: 'hi', durationMs: 5, isAborted: false, agentId: 'sub-1', usage: USAGE })
  await $.turn.complete({ turnId: 't3', answer: '', durationMs: 1, isAborted: true, usage: null })
  const meters = runs.filter((r) => r.argv[1] === 'meter')
  expect(meters.length).toBe(2)
  expect(meters[0].argv.slice(1)).toEqual(['meter', 'record'])
  expect(JSON.parse(meters[0].stdin!)).toEqual({
    project: '/work/app',
    model: 'claude-opus-5-5',
    input: 2,
    output: 7,
    cache_read: 8605,
    cache_write: 26631,
    agent: false,
    aborted: false,
    lens_before: LISTING.length,
    lens_after: REWRITTEN.length,
    requests: 2,
  })
  const sub = JSON.parse(meters[1].stdin!)
  expect(sub.agent).toBe(true)
  expect(sub.lens_before).toBeUndefined()
})

test('a failing meter never disturbs the turn', async ($, on) => {
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ deny: 'no binary' }))
  on('turn.complete', () => ({ text: 'kept' }))
  const out = await $.turn.complete({ turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(out.text).toBe('kept')
})

test('/girth-meter prints the meter for this project', async ($, on) => {
  const runs: Run[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    runs.push({ argv: e.argv })
    return { value: { exitCode: 0, stdout: 'meter: 3 turns\n', stderr: '' } }
  })
  const out = await $.command.run({ command: 'girth-meter', args: '' })
  expect(out.text).toBe('meter: 3 turns')
  expect(runs[0].argv.slice(1)).toEqual(['stats', '--meter', '--project', '/work/app'])
})

test('the commands answer even when the binary cannot run', async ($, on) => {
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ deny: 'timed out' }))
  const meter = await $.command.run({ command: 'girth-meter', args: '' })
  const xray = await $.command.run({ command: 'girth-xray', args: '' })
  expect(meter.text).toBe('girth: the meter is unavailable right now')
  expect(xray.text).toBe('girth: the x-ray is unavailable right now')
})

test('session start registers /girth-meter and warms the binary up with a long timeout', async ($, on) => {
  const names: string[] = []
  const runs: { argv: string[]; timeoutMs?: number }[] = []
  on('command.register', ($, e) => {
    names.push(e.name)
    return { value: undefined }
  })
  on('process.run', ($, e) => {
    runs.push({ argv: e.argv, timeoutMs: e.init?.timeoutMs })
    return { value: { exitCode: 0, stdout: 'girth 0.5.0\n', stderr: '' } }
  })
  on('fs.exists', () => ({ value: false }))
  on('session.start', () => ({ cwd: '/work' }))
  await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
  expect(names).toEqual(['girth-meter', 'girth-xray'])
  // A fresh install downloads the binary once; do it here, not inside Lens's 5 s budget.
  expect(runs.length).toBe(1)
  expect(runs[0].argv.slice(1)).toEqual(['--version'])
  expect(runs[0].timeoutMs).toBe(60000)
})

test('no warm-up once the binary is there', async ($, on) => {
  let runs = 0
  on('command.register', () => ({ value: undefined }))
  on('process.run', () => {
    runs += 1
    return { value: { exitCode: 0, stdout: '', stderr: '' } }
  })
  on('fs.exists', ($, e) => ({ value: e.path.endsWith('/skills/girth/bin/girth') }))
  on('session.start', () => ({ cwd: '/work' }))
  await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
  expect(runs).toBe(0)
})

test('a failed warm-up still registers the command', async ($, on) => {
  const names: string[] = []
  on('command.register', ($, e) => {
    names.push(e.name)
    return { value: undefined }
  })
  on('process.run', () => ({ deny: 'offline' }))
  on('fs.exists', () => ({ value: false }))
  on('session.start', () => ({ cwd: '/work' }))
  await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
  expect(names).toEqual(['girth-meter', 'girth-xray'])
})

const MCP = '# MCP\n\n## claude.ai Figma\nUse get_design_context.\n'
const MCP_REWRITTEN = '# MCP\n\n## Other connected servers (their instructions arrive on first use): claude.ai Figma\n'

function mcpOut() {
  return JSON.stringify({
    text: MCP_REWRITTEN,
    bytes_before: MCP.length,
    bytes_after: MCP_REWRITTEN.length,
    described: [],
    collapsed: ['claude_ai_Figma'],
    deferred: { claude_ai_Figma: '## claude.ai Figma\nUse get_design_context.\n' },
  })
}

test('MCP instructions are rewritten with --kind mcp, and held-back instructions arrive once on first use', async ($, on) => {
  const runs: Run[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    runs.push({ argv: e.argv, stdin: e.init?.stdin })
    return { value: { exitCode: 0, stdout: mcpOut(), stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  on('tool.call', () => ({ result: 'figma ok' }))
  const out = await $.prompt.attachment({ type: 'mcp_instructions_delta', text: MCP, origin: 'engine' })
  expect(out.text).toBe(MCP_REWRITTEN)
  expect(runs[0].argv.slice(1, 3)).toEqual(['lens', '--kind'])
  expect(runs[0].argv[3]).toBe('mcp')
  const first = await $.tool.call({ tool: 'mcp__claude_ai_Figma__use_figma' })
  expect(first.deny).toMatch(/^claude_ai_Figma instructions \(Girth held them back until first use/)
  expect(first.deny).toContain('Use get_design_context.')
  const second = await $.tool.call({ tool: 'mcp__claude_ai_Figma__use_figma' })
  expect(second).toEqual({ result: 'figma ok' })
  const other = await $.tool.call({ tool: 'mcp__pencil__batch_get' })
  expect(other).toEqual({ result: 'figma ok' })
  const builtin = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(builtin).toEqual({ result: 'figma ok' })
})

test('the deferred-tools list is rewritten with --kind tools', async ($, on) => {
  const kinds: string[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    kinds.push(e.argv[3])
    return { value: { exitCode: 0, stdout: JSON.stringify({ text: 'short', bytes_before: 9, bytes_after: 5 }), stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  const out = await $.prompt.attachment({ type: 'deferred_tools_delta', text: 'long list', origin: 'engine' })
  expect(out.text).toBe('short')
  expect(kinds).toEqual(['tools'])
})

test('turns record the MCP servers they called and Lens bytes summed across kinds', async ($, on) => {
  const records: any[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'meter') records.push(JSON.parse(e.init!.stdin!))
    const out = e.argv[3] === 'mcp' ? mcpOut() : lensOut()
    return { value: { exitCode: 0, stdout: out, stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  on('tool.call', () => ({ result: 'ok' }))
  on('turn.complete', () => ({ text: '' }))
  await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  await $.prompt.attachment({ type: 'mcp_instructions_delta', text: MCP, origin: 'engine' })
  await $.tool.call({ tool: 'mcp__pencil__batch_get' })
  await $.tool.call({ tool: 'mcp__claude_ai_Figma__whoami' })
  await $.turn.complete({ turnId: 't1', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  await $.turn.complete({ turnId: 't2', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(records[0].mcp).toEqual(['claude_ai_Figma', 'pencil'])
  expect(records[0].lens_before).toBe(LISTING.length + MCP.length)
  expect(records[0].lens_after).toBe(REWRITTEN.length + MCP_REWRITTEN.length)
  // Servers are counted per turn.
  expect(records[1].mcp).toBeUndefined()
})

test('/girth-xray sends the parts of the last request, with Lens sizes and the model', async ($, on) => {
  let snapshot: any = null
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'xray') {
      snapshot = JSON.parse(e.init!.stdin!)
      return { value: { exitCode: 0, stdout: 'x-ray: table\n', stderr: '' } }
    }
    return { value: { exitCode: 0, stdout: e.argv[1] === 'lens' ? lensOut() : '', stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  on('prompt.section', ($, e) => ({ text: 'section body' }))
  on('tool.describe', ($, e) => ({ description: 'Runs shell commands' }))
  on('turn.complete', () => ({ text: '' }))
  await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  await $.prompt.attachment({ type: 'todo_reminder', text: 'remember this', origin: 'engine' })
  await $.prompt.section({ name: 'memory', text: 'x' })
  await $.tool.describe({ tool: 'Bash', description: 'Runs shell commands', provider: { kind: 'builtin' } })
  await $.turn.complete({ turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  const out = await $.command.run({ command: 'girth-xray', args: '' })
  expect(out.text).toBe('x-ray: table')
  expect(snapshot.model).toBe('claude-opus-5-5')
  expect(snapshot.parts).toEqual([
    { kind: 'attachment', name: 'skill_listing', bytes: REWRITTEN.length, before: LISTING.length },
    { kind: 'attachment', name: 'todo_reminder', bytes: 13 },
    { kind: 'section', name: 'memory', bytes: 12 },
    { kind: 'tool', name: 'Bash', bytes: 19 },
  ])
})

test('a listing Lens leaves unchanged claims no savings in the meter or the x-ray', async ($, on) => {
  const records: any[] = []
  let snapshot: any = null
  let lensCalls = 0
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'meter') records.push(JSON.parse(e.init!.stdin!))
    if (e.argv[1] === 'xray') snapshot = JSON.parse(e.init!.stdin!)
    if (e.argv[1] !== 'lens') return { value: { exitCode: 0, stdout: '', stderr: '' } }
    lensCalls += 1
    // First listing is trimmed; the next one (a new listing) comes back as it was.
    const text = lensCalls === 1 ? REWRITTEN : e.init!.stdin!
    return { value: { exitCode: 0, stdout: JSON.stringify({ text, bytes_before: e.init!.stdin!.length, bytes_after: text.length }), stderr: '' } }
  })
  on('prompt.attachment', ($, e) => ({ text: e.text }))
  on('turn.complete', () => ({ text: '' }))
  await $.prompt.attachment({ type: 'skill_listing', text: LISTING, origin: 'engine' })
  await $.prompt.attachment({ type: 'skill_listing', text: 'Skills:\n- girth:girth: G.\n', origin: 'engine' })
  await $.turn.complete({ turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  await $.command.run({ command: 'girth-xray', args: '' })
  expect(records[0].lens_before).toBeUndefined()
  expect(snapshot.parts[0].before).toBeUndefined()
})

// Usage of one model request, as turn.step's result reports it.
function step(model: string, input: number, read: number, write: number) {
  return { input_tokens: input, output_tokens: 1, cache_read_input_tokens: read, cache_creation_input_tokens: write, model }
}

async function drive($: any, e: object) {
  const stream = $.turn.step(e)
  let s = await stream.next()
  while (s.done !== true) s = await stream.next()
}

test('turn.step finds cache breaks per model in the main loop, and turn.complete reports them', async ($, on) => {
  const records: any[] = []
  let now = 0
  let usage: any = null
  const line = 'girth: the prompt cache had expired (idle 2h 0m); this turn re-wrote 100K tokens (≈ $0.80)'
  on('clock.now', () => ({ value: now }))
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'meter') records.push(JSON.parse(e.init!.stdin!))
    return { value: { exitCode: 0, stdout: JSON.stringify({ keeper: { pings: 0, interval_ms: 1 }, line }), stderr: '' } }
  })
  on('turn.complete', () => ({ text: '' }))
  on('turn.step', async function* ($, e) {
    return { turnId: e.turnId, index: e.index, answer: 'ok', toolUses: [], stopReason: 'end_turn', usage }
  })
  const main = (index: number) => ({ turnId: 't1', index, model: 'claude-opus-5-5', messageCount: 1 })
  // The first request has nothing to compare with.
  now = 60_000
  usage = step('claude-opus-5-5', 2, 10_000, 100_000)
  await drive($, main(0))
  // Two hours later the cache is gone: 100,000 of the 110,000 cached tokens
  // are lost, and the 95,000 written count as re-written.
  now = 7_260_000
  usage = step('claude-opus-5-5', 2, 10_000, 95_000)
  await drive($, main(1))
  // A background request on another model is compared only with its own model.
  usage = step('claude-haiku-5-5', 2, 0, 5_000)
  await drive($, { ...main(2), model: 'claude-haiku-5-5' })
  // Under 5,000 tokens lost is no break, even at 80% of what was cached.
  usage = step('claude-haiku-5-5', 2, 1_000, 3_000)
  await drive($, { ...main(2), model: 'claude-haiku-5-5' })
  // A healthy request reads back what the last one wrote.
  usage = step('claude-opus-5-5', 2, 130_000, 1_000)
  await drive($, main(3))
  // Under 20% of what was cached is no break, even at 6,000 tokens.
  usage = step('claude-opus-5-5', 2, 125_000, 6_000)
  await drive($, main(3))
  // A subagent's requests are its own conversation.
  usage = step('claude-opus-5-5', 2, 0, 200_000)
  await drive($, { ...main(4), agentId: 'sub-1' })
  // A prompt that shrank (compaction) is not a break.
  usage = step('claude-opus-5-5', 2, 0, 20_000)
  await drive($, main(5))
  const out = await $.turn.complete({ turnId: 't1', answer: 'hi', durationMs: 5, isAborted: false, usage: USAGE })
  expect(records[0].breaks).toEqual([{ lost: 100_000, rewritten: 95_000, idle_s: 7_200 }])
  // The conversation's size for the keeper: the last main request's prompt.
  expect(records[0].context).toBe(20_002)
  expect(out.text).toBe(line)
  // Breaks belong to their turn.
  usage = step('claude-opus-5-5', 2, 20_000, 500)
  await drive($, { ...main(0), turnId: 't2' })
  await $.turn.complete({ turnId: 't2', answer: 'hi', durationMs: 5, isAborted: false, usage: USAGE })
  expect(records[1].breaks).toBeUndefined()
})

test('the doctor line is left out when there is none, and bad meter output changes nothing', async ($, on) => {
  let stdout = JSON.stringify({ keeper: { pings: 0, interval_ms: 1 } })
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ value: { exitCode: 0, stdout, stderr: '' } }))
  on('turn.complete', () => ({ text: 'other mod' }))
  const done = { turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE }
  expect((await $.turn.complete(done)).text).toBe('other mod')
  stdout = 'not json'
  expect((await $.turn.complete(done)).text).toBe('other mod')
  // Another mod's line and ours are both shown.
  stdout = JSON.stringify({ keeper: { pings: 0, interval_ms: 1 }, line: 'girth: x' })
  expect((await $.turn.complete(done)).text).toBe('other mod\ngirth: x')
})

import { compactor, keeper, ping } from '../girth.js'

const FORK_USAGE = { input_tokens: 3, output_tokens: 1, cache_read_input_tokens: 200_000, cache_creation_input_tokens: 0 }

test('a ping forks the conversation, records its usage, and says whether the cache was warm', async () => {
  const recorded: any[] = []
  const prompts: any[] = []
  const deps = {
    fork: async (o: any) => { prompts.push(o); return { isAnswered: true, text: '.', usage: FORK_USAGE } },
    record: async (p: any) => { recorded.push(p) },
  }
  expect(await ping(deps, 200_100)).toBe(true)
  expect(prompts[0].prompt).toMatch(/one character/)
  expect(recorded).toEqual([{ input: 3, output: 1, read: 200_000, write: 0, expected: 200_100 }])
  // Read under half of what was cached: the cache was already gone.
  expect(await ping(deps, 400_001)).toBe(false)
  expect(recorded.length).toBe(2)
  // Exactly half still counts as warm.
  expect(await ping(deps, 400_000)).toBe(true)
  // An aborted fork still spent tokens: recorded, and judged by what it read.
  const aborted = { isAnswered: false, reason: 'aborted', usage: { ...FORK_USAGE, cache_read_input_tokens: 0 } }
  expect(await ping({ ...deps, fork: async () => aborted }, 10)).toBe(false)
  expect(recorded.length).toBe(4)
  // No usage, or an error: nothing recorded, not warm.
  expect(await ping({ ...deps, fork: async () => ({ isAnswered: false, reason: 'nothing-to-fork' }) }, 1)).toBe(false)
  expect(await ping({ ...deps, fork: async () => { throw new Error('refused') } }, 1)).toBe(false)
  expect(recorded.length).toBe(4)
})

function fakeClock() {
  const timers: { ms: number; fn: () => Promise<void>; cancelled: boolean }[] = []
  return {
    timers,
    after: (ms: number, fn: () => Promise<void>) => {
      const t = { ms, fn, cancelled: false }
      timers.push(t)
      return { cancel: () => { t.cancelled = true } }
    },
  }
}

test('the keeper pings one at a time, stops after the plan or a miss, and stop() cancels', async () => {
  const clock = fakeClock()
  let read = 200_000
  const recorded: any[] = []
  const k = keeper({
    after: clock.after,
    fork: async () => ({ isAnswered: true, text: '.', usage: { ...FORK_USAGE, cache_read_input_tokens: read } }),
    record: async (p: any) => { recorded.push(p) },
  })
  k.start({ pings: 2, interval_ms: 3_300_000 }, 200_000)
  expect(clock.timers.map((t) => t.ms)).toEqual([3_300_000])
  await clock.timers[0].fn()
  expect(clock.timers.length).toBe(2)
  await clock.timers[1].fn()
  // Two pings planned, two sent: no third timer.
  expect(clock.timers.length).toBe(2)
  expect(recorded.length).toBe(2)
  // A miss ends the idle period's pings.
  k.start({ pings: 3, interval_ms: 10 }, 200_000)
  read = 1_000
  await clock.timers[2].fn()
  expect(clock.timers.length).toBe(3)
  // Starting again, or stopping, cancels the pending timer.
  read = 200_000
  k.start({ pings: 3, interval_ms: 10 }, 200_000)
  k.start({ pings: 3, interval_ms: 10 }, 200_000)
  expect(clock.timers[3].cancelled).toBe(true)
  k.stop()
  expect(clock.timers[4].cancelled).toBe(true)
  // A zero plan arms nothing.
  k.start({ pings: 0, interval_ms: 10 }, 200_000)
  expect(clock.timers.length).toBe(5)
})

// The harness can't observe a timer's cancel() (it runs client-side); the
// keeper test above covers stop(), and turn.start calls it.
test('main turns arm the keeper with the meter plan', async ($, on) => {
  const armed: number[] = []
  let pings = 3
  on('clock.now', () => ({ value: 0 }))
  on('clock.after', ($, e) => {
    armed.push(e.ms)
    return { value: {} }
  })
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ value: { exitCode: 0, stdout: JSON.stringify({ keeper: { pings, interval_ms: 3_300_000 } }), stderr: '' } }))
  on('turn.complete', () => ({ text: '' }))
  on('turn.start', ($, e) => e)
  on('turn.step', async function* ($, e) {
    return { turnId: e.turnId, index: 0, answer: 'ok', toolUses: [], stopReason: 'end_turn', usage: step('claude-opus-5-5', 2, 100_000, 20_000) }
  })
  await drive($, { turnId: 't1', index: 0, model: 'claude-opus-5-5', messageCount: 1 })
  await $.turn.complete({ turnId: 't1', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(armed).toEqual([3_300_000])
  // A subagent's turn doesn't arm.
  await $.turn.start({ turnId: 's', agentId: 'sub-1', prompt: 'x' })
  await $.turn.complete({ turnId: 's', answer: 'a', durationMs: 1, isAborted: false, agentId: 'sub-1', usage: USAGE })
  expect(armed.length).toBe(1)
  // A new main turn re-arms after it completes.
  await $.turn.start({ turnId: 't2', prompt: 'x' })
  await $.turn.complete({ turnId: 't2', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(armed).toEqual([3_300_000, 3_300_000])
  // A zero plan arms nothing.
  pings = 0
  await $.turn.complete({ turnId: 't3', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(armed.length).toBe(2)
})

test('the compactor compacts once after the interval, records it, and leaves a note for the next answer', async () => {
  const clock = fakeClock()
  const recorded: any[] = []
  let answer: any = { messages: [{}, {}], tokensBefore: 583_000, tokensAfter: 21_000 }
  let calls = 0
  const c = compactor({
    after: clock.after,
    compact: async () => {
      calls += 1
      if (answer instanceof Error) throw answer
      return answer
    },
    record: async (r: any) => { recorded.push(r) },
  })
  expect(c.takeNote()).toBe(null)
  c.start({ pings: 0, interval_ms: 3_300_000, compact: true }, 600_000)
  expect(clock.timers.map((t) => t.ms)).toEqual([3_300_000])
  await clock.timers[0].fn()
  expect(calls).toBe(1)
  expect(recorded).toEqual([{ before: 583_000, after: 21_000 }])
  expect(c.takeNote()).toEqual({ before: 583_000, after: 21_000 })
  // The note is given once.
  expect(c.takeNote()).toBe(null)
  // One compaction per idle period: no second timer.
  expect(clock.timers.length).toBe(1)
  // Without the engine's counts, the context Girth saw stands in for "before",
  // and nothing goes to the ledger without an "after".
  answer = { messages: [] }
  c.start({ pings: 0, interval_ms: 10, compact: true }, 400_000)
  await clock.timers[1].fn()
  expect(c.takeNote()).toEqual({ before: 400_000 })
  expect(recorded.length).toBe(1)
  // A veto, a refusal or an error: no record, no note, nothing thrown.
  for (const a of [{ skip: 'not now' }, new Error('headless'), undefined]) {
    answer = a
    c.start({ pings: 0, interval_ms: 10, compact: true }, 400_000)
    await clock.timers[clock.timers.length - 1].fn()
    expect(c.takeNote()).toBe(null)
  }
  expect(recorded.length).toBe(1)
  // stop() and a new start() cancel the pending timer, and a stale callback does nothing.
  const before = calls
  c.start({ pings: 0, interval_ms: 10, compact: true }, 400_000)
  const stale = clock.timers[clock.timers.length - 1]
  c.stop()
  expect(stale.cancelled).toBe(true)
  await stale.fn()
  expect(calls).toBe(before)
  c.start({ pings: 0, interval_ms: 10, compact: true }, 400_000)
  const first = clock.timers[clock.timers.length - 1]
  c.start({ pings: 0, interval_ms: 10, compact: true }, 400_000)
  expect(first.cancelled).toBe(true)
  await first.fn()
  expect(calls).toBe(before)
  // A failing record still leaves the note.
  const failing = compactor({
    after: clock.after,
    compact: async () => ({ tokensBefore: 5, tokensAfter: 1 }),
    record: async () => { throw new Error('disk full') },
  })
  failing.start({ pings: 0, interval_ms: 10, compact: true }, 9)
  await clock.timers[clock.timers.length - 1].fn()
  expect(failing.takeNote()).toEqual({ before: 5, after: 1 })
})

test('a compact plan arms the compactor, not the keeper, and only for the main loop', async ($, on) => {
  const armed: number[] = []
  const sent: any[] = []
  on('clock.now', () => ({ value: 0 }))
  on('clock.after', ($, e) => {
    armed.push(e.ms)
    return { value: {} }
  })
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    sent.push(e.init?.stdin ? JSON.parse(e.init.stdin) : null)
    return { value: { exitCode: 0, stdout: JSON.stringify({ keeper: { pings: 0, interval_ms: 3_300_000, compact: true } }), stderr: '' } }
  })
  on('turn.complete', () => ({ text: '' }))
  on('turn.start', ($, e) => e)
  on('turn.step', async function* ($, e) {
    return { turnId: e.turnId, index: 0, answer: 'ok', toolUses: [], stopReason: 'end_turn', usage: step('claude-opus-5-5', 2, 500_000, 20_000) }
  })
  await drive($, { turnId: 't1', index: 0, model: 'claude-opus-5-5', messageCount: 1 })
  await $.turn.complete({ turnId: 't1', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(armed).toEqual([3_300_000])
  // No compaction happened yet, so the next turn carries no note.
  expect(sent[0].compacted).toBe(undefined)
  await $.turn.complete({ turnId: 's', answer: 'a', durationMs: 1, isAborted: false, agentId: 'sub-1', usage: USAGE })
  expect(armed.length).toBe(1)
})

test('the line the meter returns after a compaction goes under the answer', async ($, on) => {
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ value: { exitCode: 0, stdout: JSON.stringify({ keeper: { pings: 0, interval_ms: 1 }, line: 'compacted this conversation while you were away (583K → 21K tokens)' }), stderr: '' } }))
  on('turn.complete', () => ({ text: '' }))
  const out = await $.turn.complete({ turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(out.text).toBe('compacted this conversation while you were away (583K → 21K tokens)')
})

function spawnInput(over: any = {}) {
  return {
    tool_use_id: 'toolu_1',
    prompt: 'look around',
    description: 'explore',
    subagentType: 'Explore',
    provider: { plugin: 'engine', tier: 'core' },
    parentModel: 'claude-opus-5-5',
    ...over,
  }
}

test('built-in subagents run on the configured model; explicit models, plugin agents and forks keep theirs', async ($, on) => {
  const runs: string[][] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    runs.push(e.argv)
    return { value: { exitCode: 0, stdout: '{"agents":{"model":"haiku"}}\n', stderr: '' } }
  })
  // Stands in for the engine: what the subagent ends up on (`inherit` when no model was set).
  on('agent.spawn', ($, e) => ({ model: e.model ?? 'inherit' }))
  expect((await $.agent.spawn(spawnInput())).model).toBe('haiku')
  expect((await $.agent.spawn(spawnInput({ subagentType: 'general-purpose' }))).model).toBe('haiku')
  // The settings are read once per session.
  expect(runs.length).toBe(1)
  expect(runs[0].slice(1)).toEqual(['settings', '--project', '/work/app'])
  expect((await $.agent.spawn(spawnInput({ model: 'opus' }))).model).toBe('opus')
  expect((await $.agent.spawn(spawnInput({ provider: { plugin: 'superpowers', tier: 'user' } }))).model).toBe('inherit')
  expect((await $.agent.spawn(spawnInput({ subagentType: 'fork' }))).model).toBe('inherit')
})

test('no subagent model configured, or settings that fail, change nothing', async ($, on) => {
  let stdout = '{}\n'
  let exitCode = 0
  let runs = 0
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => {
    runs += 1
    return { value: { exitCode, stdout, stderr: '' } }
  })
  on('agent.spawn', ($, e) => ({ model: e.model ?? 'inherit' }))
  expect((await $.agent.spawn(spawnInput())).model).toBe('inherit')
  expect((await $.agent.spawn(spawnInput())).model).toBe('inherit')
  expect(runs).toBe(1)
})

test('a settings command that fails or prints garbage leaves subagents alone', async ($, on) => {
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ value: { exitCode: 0, stdout: 'not json', stderr: '' } }))
  on('agent.spawn', ($, e) => ({ model: e.model ?? 'inherit' }))
  expect((await $.agent.spawn(spawnInput())).model).toBe('inherit')
})

test('a settings command that exits non-zero leaves subagents alone', async ($, on) => {
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ value: { exitCode: 2, stdout: '{"agents":{"model":"haiku"}}', stderr: '' } }))
  on('agent.spawn', ($, e) => ({ model: e.model ?? 'inherit' }))
  expect((await $.agent.spawn(spawnInput())).model).toBe('inherit')
})

test("the line goes under the answer without repeating it, and after another mod's line", async ($, on) => {
  const line = 'compacted this conversation while you were away (583K → 21K tokens)'
  let theirs: string | null = null
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', () => ({ value: { exitCode: 0, stdout: JSON.stringify({ keeper: { pings: 0, interval_ms: 1 }, line }), stderr: '' } }))
  // The engine resolves turn.complete to the answer itself; another mod's line replaces it.
  on('turn.complete', ($, e) => ({ text: theirs ?? e.answer }))
  const out = await $.turn.complete({ turnId: 't', answer: 'HELIOTROPE KIWI', durationMs: 1, isAborted: false, usage: USAGE })
  expect(out.text).toBe(line)
  theirs = 'tl;dr: done'
  const both = await $.turn.complete({ turnId: 't2', answer: 'HELIOTROPE KIWI', durationMs: 1, isAborted: false, usage: USAGE })
  expect(both.text).toBe('tl;dr: done\n' + line)
})

test('Bash calls go to the meter with their turn, and their results pass through unchanged', async ($, on) => {
  // A test's tool.call answer can only be { result }, and a test's own call
  // carries no agentId, so the error path (core's isError and text) and a
  // subagent's own calls are checked live, not here.
  const records: any[] = []
  const LONG = 'x'.repeat(5000) + 'test result: ok. 1 passed'
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'meter') records.push(JSON.parse(e.init!.stdin!))
    return { value: { exitCode: 0, stdout: '{}', stderr: '' } }
  })
  on('tool.call', ($, e) => {
    if (e.command === 'sleep 600') return { result: { stdout: '', stderr: '', interrupted: false, backgroundTaskId: 'b1' } }
    if (e.command === 'make') return { result: { stdout: 'built', stderr: 'warn', interrupted: true } }
    return { result: { stdout: LONG, stderr: '', interrupted: false } }
  })
  on('turn.complete', () => ({ text: '' }))
  const passed = await $.tool.call({ tool: 'Bash', command: 'cargo test | tail -1' })
  expect((passed.result as any).stdout).toBe(LONG)
  await $.tool.call({ tool: 'Bash', command: 'sleep 600' })
  await $.tool.call({ tool: 'Bash', command: 'make' })
  await $.tool.call({ tool: 'Read', file_path: 'a.md' } as any)
  await $.turn.complete({ turnId: 's', answer: 'a', durationMs: 1, isAborted: false, agentId: 'sub-1', usage: USAGE })
  await $.turn.complete({ turnId: 't1', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  await $.turn.complete({ turnId: 't2', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  const tail = LONG.slice(-4000)
  // A subagent's turn does not take the main loop's calls.
  expect(records[0].bash).toBeUndefined()
  expect(records[1].bash).toEqual([
    { command: 'cargo test | tail -1', output: tail, error: false, background: false },
    { command: 'sleep 600', output: '', error: false, background: true },
    { command: 'make', output: 'built\nwarn', error: false, background: true },
  ])
  // Calls are counted per turn, and a turn without usage drops its own.
  expect(records[2].bash).toBeUndefined()
  await $.tool.call({ tool: 'Bash', command: 'git commit -m aborted' })
  await $.turn.complete({ turnId: 't3', answer: '', durationMs: 1, isAborted: true, usage: null })
  await $.turn.complete({ turnId: 't4', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(records[3].bash).toBeUndefined()
})

test('at most 64 Bash calls a turn go to the meter', async ($, on) => {
  const records: any[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'meter') records.push(JSON.parse(e.init!.stdin!))
    return { value: { exitCode: 0, stdout: '{}', stderr: '' } }
  })
  on('tool.call', () => ({ result: { stdout: '', stderr: '', interrupted: false } }))
  on('turn.complete', () => ({ text: '' }))
  for (let i = 0; i < 65; i++) await $.tool.call({ tool: 'Bash', command: 'echo ' + i })
  await $.turn.complete({ turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(records[0].bash.length).toBe(64)
  expect(records[0].bash[63].command).toBe('echo 63')
})

test('a denied Bash call is not recorded, and the output tail never starts inside a character', async ($, on) => {
  const records: any[] = []
  on('session.cwd', () => ({ value: '/work/app' }))
  on('process.run', ($, e) => {
    if (e.argv[1] === 'meter') records.push(JSON.parse(e.init!.stdin!))
    return { value: { exitCode: 0, stdout: '{}', stderr: '' } }
  })
  on('tool.call', ($, e) => {
    if (e.command === 'git commit -m blocked') return { deny: 'not allowed' }
    return { result: { stdout: '🎉'.repeat(2001) + 'x', stderr: '', interrupted: false } }
  })
  on('turn.complete', () => ({ text: '' }))
  const denied = await $.tool.call({ tool: 'Bash', command: 'git commit -m blocked' })
  expect(denied.deny).toBe('not allowed')
  await $.tool.call({ tool: 'Bash', command: 'cargo test' })
  await $.turn.complete({ turnId: 't', answer: 'a', durationMs: 1, isAborted: false, usage: USAGE })
  expect(records[0].bash.length).toBe(1)
  const output: string = records[0].bash[0].output
  expect(output.endsWith('🎉x')).toBe(true)
  const first = output.charCodeAt(0)
  expect(first >= 0xdc00 && first <= 0xdfff).toBe(false)
})
