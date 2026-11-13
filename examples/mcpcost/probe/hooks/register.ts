import type { Register } from 'claude-code';
const rows: string[] = [];
// Where the rows go: change this to a folder of your own.
const OUT = '/tmp/girth-mcpcost-probe.jsonl';
async function flush($: any) { await $.fs.write(OUT, rows.join('\n') + '\n').catch(() => {}); }
export const register: Register = (on) => {
  on('telemetry.log', { to: 'collector' }, async ($, e, next) => {
    try {
      if (e.event === 'api_request') {
        const a: any = e.attributes;
        rows.push(JSON.stringify({ k: 'req', qs: a.query_source, inp: a.input_tokens, cr: a.cache_read_tokens, cw: a.cache_creation_tokens }));
        await flush($);
      }
    } catch {}
    return next(e);
  }).catch(async ($: any, e: any, next: any) => next(e));
  on('turn.complete', async ($, e, next) => {
    const r = await next(e);
    try {
      for (const detail of ['summary', 'full'] as const) {
        const u: any = await $.session.usage({ breakdown: detail });
        const b = u.context?.breakdown;
        rows.push(JSON.stringify({ k: 'ctx', detail, agent: e.agentId ?? null, tokens: u.context?.tokens,
          categories: b?.categories?.map((c: any) => [c.name, c.kind, c.tokens]),
          mcp: b?.mcpTools?.map((t: any) => [t.serverName, t.name, t.tokens, t.isLoaded]),
          skills: b?.skills ? { tokens: b.skills.tokens, n: b.skills.skills?.length } : null,
          memory: b?.memoryFiles?.map((m: any) => m.tokens), agents: b?.agents?.length }));
      }
      await flush($);
    } catch (err) { rows.push(JSON.stringify({ k: 'err', msg: String(err) })); await flush($); }
    return r;
  }).catch(async ($: any, e: any, next: any) => next(e));
};
