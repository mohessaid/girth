// Girth's mod. Lens: the skill listing Claude reads keeps the descriptions this
// project needs and lists every other skill by name. Meter: each turn's token
// usage goes to the local ledger. The girth binary makes every decision, and
// every hook fails open: on any error Claude Code carries on unchanged.

const TIMEOUT_MS = 5000

// Listing text -> the rewrite, so a session always sends the same listing and
// the prompt cache stays warm.
const rewrites = new Map()
// Byte counts of the listing rewrite now in effect, for the meter.
let lens = null

function launcher($) {
  return $.plugin.root + '/skills/girth/scripts/girth'
}

export function register(on) {
  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'girth-meter',
      description: 'Show token usage and Lens savings for this project',
    })
    return next(e)
  }).catch(async ($, e, next) => next(e))

  on('prompt.attachment', { type: 'skill_listing' }, async ($, e, next) => {
    let rewrite = rewrites.get(e.text)
    if (rewrite === undefined) {
      const cwd = await $.session.cwd()
      const run = await $.process.run([launcher($), 'lens', '--project', cwd, '--json'], {
        stdin: e.text,
        timeoutMs: TIMEOUT_MS,
      })
      if (run.exitCode !== 0) return next(e)
      const out = JSON.parse(run.stdout)
      if (typeof out.text !== 'string') return next(e)
      rewrite = { text: out.text, before: out.bytes_before, after: out.bytes_after }
      rewrites.set(e.text, rewrite)
    }
    lens = rewrite.text === e.text ? null : { before: rewrite.before, after: rewrite.after }
    return next({ ...e, text: rewrite.text })
  }).catch(async ($, e, next) => next(e))

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    const usage = e.usage
    if (!usage) return result
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
    if (lens && e.agentId === undefined) {
      turn.lens_before = lens.before
      turn.lens_after = lens.after
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
}
