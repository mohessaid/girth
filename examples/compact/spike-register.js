let timer
export function register(on) {
  on('turn.start', async ($, e, next) => {
    timer?.cancel()
    return next(e)
  })
  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId !== undefined) return result
    timer?.cancel()
    timer = $.clock.after(20000, async () => {
      const before = await $.session.usage()
      let outcome = 'none'
      try {
        const r = await $.session.compact({ instructions: 'Keep the project codename and the numbers the user gave.' })
        outcome = r?.skip ? 'skip:' + r.skip : 'compacted ' + (r?.tokensBefore ?? '?') + '->' + (r?.tokensAfter ?? '?') + ' msgs ' + (r?.messages?.length ?? '?')
      } catch (err) {
        outcome = 'error:' + String(err?.message ?? err)
      }
      const after = await $.session.usage()
      await $.fs.write($.plugin.root + '/../outcome.txt', JSON.stringify({ outcome, before: before.context?.tokens, after: after.context?.tokens }))
    })
    return result
  })
}
