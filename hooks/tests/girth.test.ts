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
  expect(runs[0].argv.slice(1)).toEqual(['lens', '--project', '/work/app', '--json'])
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
  expect(names).toEqual(['girth-meter'])
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
  expect(names).toEqual(['girth-meter'])
})
