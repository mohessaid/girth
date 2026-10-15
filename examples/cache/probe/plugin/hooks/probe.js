// Probe: after each main turn, fork once after PROBE_PING_MS and log the fork's usage.
export function register(on) {
  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId !== undefined) return result
    const ms = Number((await $.env.get('PROBE_PING_MS')) ?? '0')
    const log = await $.env.get('PROBE_LOG')
    if (ms > 0 && log) {
      $.clock.after(ms, async () => {
        const t = Date.now()
        let out
        try {
          const r = await $.model.fork({ prompt: 'Reply with one character: .' })
          out = { at: t, isAnswered: r.isAnswered, reason: r.reason, usage: r.usage }
        } catch (err) {
          out = { at: t, error: String(err) }
        }
        await $.fs.write(log, JSON.stringify(out) + '\n')
      })
    }
    return result
  })
}
