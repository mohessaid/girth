"""Usage: python3 examples/compact/idle_sim.py [idle_seconds=3300] [min_context=150000]

Idle compaction, simulated on the last 7 days of ~/.girth/ledger.jsonl (main-loop Claude Code turns).
A compaction fires after a turn when the next gap (or the session's end) is >= IDLE and the
context is >= MIN. It costs one warm read of the context plus a ~3k-token summary, and leaves
AFTER tokens. Later turns read `offset` fewer tokens per request; a rewrite (Turn.breaks) on
the return writes `offset` fewer tokens. A real drop in context (a compaction or /clear) resets it."""
import json,os,time,collections,sys
IDLE=int(sys.argv[1]) if len(sys.argv) > 1 else 3300; MIN=int(sys.argv[2]) if len(sys.argv) > 2 else 150000; AFTER=30000
# Optional: also compact after a commit turn when the next turn is >= COMMIT_GAP seconds away.
COMMIT_GAP=int(sys.argv[3]) if len(sys.argv) > 3 else None
COMMIT_MIN=int(sys.argv[4]) if len(sys.argv) > 4 else MIN
# Optional: also compact between turns whenever the context reaches CAP tokens.
CAP=int(sys.argv[5]) if len(sys.argv) > 5 else None
now=time.time(); start=now-7*86400
rates={'opus':(4,20,0.2,6.25,8),'sonnet':(2,10,0.1,2.5,4),'haiku':(0.1,0.5,0.01,0.125,0.2),'fable':(10,50,0.25,12.5,20)}
def R(m):
    for k,v in rates.items():
        if k in m: return v
S=collections.defaultdict(list)
for l in open(os.path.expanduser('~/.girth/ledger.jsonl')):
    try: d=json.loads(l)
    except: continue
    if d.get('kind')=='turn' and d['ts']>=start and not d.get('agent') and not d.get('harness') and d.get('session'):
        S[d['session']].append(d)
ctx=lambda d:(d['cache_read']+d['cache_write']+d['input'])/max(d.get('requests') or 1,1)
saved=cost=0; fires=0; reads_after_idle=reads_cont=0
for s,ts in S.items():
    ts.sort(key=lambda d:d['ts']); offset=0
    for i,cur in enumerate(ts):
        r=R(cur['model']);
        if not r: continue
        req=max(cur.get('requests') or 1,1)
        gap_before=cur['ts']-ts[i-1]['ts'] if i else 0
        rd=cur['cache_read']*r[2]/1e6
        if gap_before>=IDLE: reads_after_idle+=rd
        else: reads_cont+=rd
        if i and ctx(cur)<0.8*ctx(ts[i-1]): offset=0   # a real compaction or /clear
        if offset>0:
            rewritten=sum(b.get('rewritten',0) for b in cur.get('breaks',[]))
            w=min(rewritten,offset)
            saved+=w*r[4]/1e6 + max(0,offset*req-w)*r[2]/1e6
        gap_after=(ts[i+1]['ts']-cur['ts']) if i+1<len(ts) else 10**9
        c=ctx(cur)-offset
        at_commit = COMMIT_GAP is not None and 'commit' in cur.get('outcomes',[]) and gap_after>=COMMIT_GAP and c>=COMMIT_MIN
        at_cap = CAP is not None and c >= CAP
        if (gap_after>=IDLE and c>=MIN) or at_commit or at_cap:
            fires+=1; cost+=c*r[2]/1e6+3000*r[1]/1e6
            if i+1<len(ts): offset+=c-AFTER
extra = (f" + commit gap>={COMMIT_GAP}s min={COMMIT_MIN//1000}k" if COMMIT_GAP is not None else "") + (f" + cap {CAP//1000}k" if CAP else "")
print(f"idle>={IDLE}s min={MIN//1000}k{extra}: {fires} compactions cost ${cost:,.0f}, later reads/rewrites cut ${saved:,.0f}, net ${saved-cost:,.0f}")
print(f"  cache reads: ${reads_after_idle:,.0f} in turns right after an idle gap, ${reads_cont:,.0f} in continuous work")
