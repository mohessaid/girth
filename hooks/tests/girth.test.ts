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
