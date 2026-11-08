import type { Register } from 'claude-code';
const rows: string[] = [];
// Where the rows go: change this to a folder of your own.
const OUT = '/tmp/girth-sidequery-probe.jsonl';
async function flush($: any) { await $.fs.write(OUT, rows.join('\n') + '\n').catch(() => {}); }
export const register: Register = (on) => {
  on('telemetry.log', { to: 'collector' }, async ($, e, next) => {
    try {
      if (e.event === 'api_request') {
        const a: any = e.attributes;
        rows.push(JSON.stringify({ k: 'req', qs: a.query_source, model: a.model, inp: a.input_tokens, out: a.output_tokens, cr: a.cache_read_tokens, cw: a.cache_creation_tokens, cw5: a.cache_creation_5m_tokens, cw1: a.cache_creation_1h_tokens, micros: a.cost_usd_micros, at: e.loggedAt }));
        await flush($);
      }
    } catch {}
    return next(e);
  }).catch(async ($: any, e: any, next: any) => next(e));
  on('turn.complete', async ($, e, next) => {
    const r = await next(e);
    try { rows.push(JSON.stringify({ k: 'turn', agent: e.agentId ?? null, usage: e.usage })); await flush($); } catch {}
    return r;
  }).catch(async ($: any, e: any, next: any) => next(e));
};
