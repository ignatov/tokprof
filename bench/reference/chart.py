#!/usr/bin/env python3
"""Chart of reference-bench runs: what getting back to work costs for each earlier session, when
resuming it (tokprof's forecast of its first request) and in a new session that refers to it
(measured).

Usage: chart.py OUT.svg RESULTS.json [RESULTS.json...] [--sessions NAME[:AGENT],...] [--title T] [--subtitle S]
It also writes OUT-dark.svg, the same chart for dark themes. --sessions picks the earlier
sessions by name (for example codex-01a0eec6) and their order. NAME:AGENT charts the runs in
which that agent picked up the session, for example codex-01a0eec6:claude.
"""
import json
import os
import sys
from xml.sax.saxutils import escape

# The palette of the `tokprof bench` chart: resume keeps its blue, the new session its aqua, and
# a new session in the other agent takes the orange between them.
THEMES = {
    'light': dict(surface='#fcfcfb', grid='#e1e0d9', axis='#c3c2b7', ink='#0b0b0b', ink2='#52514e', muted='#898781',
                  resume='#2a78d6', fresh='#1baf7a', other='#eb6834'),
    'dark': dict(surface='#1a1a19', grid='#2c2c2a', axis='#383835', ink='#f0efec', ink2='#c3c2b7', muted='#898781',
                 resume='#3987e5', fresh='#199e70', other='#d95926'),
}
AGENTS = {'claude': 'Claude Code', 'codex': 'Codex'}
# The models a new session gets when the run names none.
DEFAULT_MODELS = {'claude': 'claude-opus-5-5', 'codex': 'gpt-6-astra'}
TITLE = 'Getting back to a cold session: resume it, or refer to it from a new one'
SUBTITLE = 'Cost of picking up the work, API-equivalent. A new session got a reference and a read-only task: what was done, what is left'
LABELS = {'resume': 'resume it', 'id': 'id only', 'path': 'id + log path', 'note': 'id + note'}
W, PAD, ROW, BAR = 880, 24, 28, 18
FONT = "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"


def rounded(tokens):
    """953.4k as 953k, 1.2M as is."""
    return f"{float(tokens[:-1]):.0f}k" if tokens.endswith('k') else tokens


def dollars(s):
    return float(s.replace('$', '').replace(',', ''))


def bar(x0, y, width, color):
    """A bar with a 4px rounded end, square at the baseline."""
    r = min(4.0, width / 2, BAR / 2)
    return (f'<path d="M{x0:.1f},{y:.1f} h{width - r:.1f} a{r},{r} 0 0 1 {r},{r} v{BAR - 2 * r:.1f} '
            f'a{r},{r} 0 0 1 -{r},{r} h-{width - r:.1f} z" fill="{color}"/>')


def header(agent, old):
    """The title of a group: the agent, then why the old session was picked, how long it ran, its
    requests, its context and its log."""
    if not old:
        return AGENTS[agent], ''
    parts = []
    if old.get('span'):
        parts.append(f"ran for {old['span'].replace('d', ' days').replace('m', ' minutes').replace('h', ' hours')}")
    if old.get('requests'):
        parts.append(f"{int(old['requests']):,} requests")
    if old.get('context'):
        parts.append(f"context {rounded(old['context'])} tokens")
    if old.get('log_mb'):
        mb = old['log_mb']
        parts.append(f"log {mb / 1000:.1f} GB" if mb >= 1000 else f"log {mb:.0f} MB" if mb >= 10 else f"log {mb:.1f} MB")
    return AGENTS[agent], f" · {old.get('pick') or 'old session'}: " + ', '.join(parts) if parts else ''


def svg(rows, specs, t, title=TITLE, subtitle=SUBTITLE):
    groups = []
    for spec in specs:
        name, _, by = spec.partition(':')
        # Every results file of a session has its resume row; the one that says why the session was
        # picked comes first.
        resume = sorted((r for r in rows if r['name'] == name and r['variant'] == 'resume'), key=lambda r: not r.get('pick'))[:1]
        runs = [r for r in rows if r['name'] == name and r['variant'] != 'resume' and r['by'] == (by or r['agent'])]
        groups.append((name, resume + runs))
    shown = [r for _, g in groups for r in g]
    top = max(dollars(r['spend']) for r in shown)
    step = next(s for s in (0.5, 1, 2, 5, 10, 20, 50) if top / s <= 5)
    xmax = top * 1.06
    x0, x1 = PAD + 150, W - PAD - 96
    x = lambda v: x0 + (x1 - x0) * v / xmax
    y_rows = 112
    tops = [y_rows]
    for _, g in groups:
        tops.append(tops[-1] + 34 + ROW * len(g))
    h_plot = tops[-1]
    H = h_plot + 64
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" '
         f'aria-labelledby="title desc" font-family="{FONT}">']
    o.append(f'<title id="title">{escape(title)}</title>')
    desc = '; '.join(f"{AGENTS[g[0]['agent']]} {n}: " + ', '.join(f"{LABELS[r['variant']]}{' by ' + AGENTS[r['by']] if r['by'] != r['agent'] else ''} {r['spend']}" for r in g) for n, g in groups)
    o.append(f'<desc id="desc">{escape(desc)}</desc>')
    o.append('<style>.num{font-variant-numeric:tabular-nums}</style>')
    o.append(f'<rect width="{W}" height="{H}" rx="10" fill="{t["surface"]}"/>')
    o.append(f'<text x="{PAD}" y="34" font-size="17" font-weight="600" fill="{t["ink"]}">{escape(title)}</text>')
    o.append(f'<text x="{PAD}" y="56" font-size="12.5" fill="{t["ink2"]}">{escape(subtitle)}</text>')
    same = any(r['variant'] != 'resume' and r['by'] == r['agent'] for r in shown)
    other = any(r['by'] != r['agent'] for r in shown)
    legend = [('resume', 'resume: tokprof’s forecast of the first request')]
    if same:
        legend.append(('fresh', 'new session, same agent: measured' if other else 'new session that refers to it: measured'))
    if other:
        legend.append(('other', 'new session in the other agent: measured'))
    lx = PAD
    for key, label in legend:
        o.append(f'<rect x="{lx}" y="74" width="12" height="12" rx="2" fill="{t[key]}"/>')
        o.append(f'<text x="{lx + 18}" y="84.5" font-size="12.5" fill="{t["ink2"]}">{label}</text>')
        lx += 18 + 0.5 * 12.5 * len(label) + 28
    ticks = []
    v = 0.0
    while v <= xmax + 1e-9:
        ticks.append(v)
        tick = f'${v:.0f}' if step >= 1 else f'${v:.2f}'
        o.append(f'<text class="num" x="{x(v):.1f}" y="{h_plot + 12}" font-size="11.5" text-anchor="middle" fill="{t["ink2"]}">{tick}</text>')
        v += step
    for gi, (name, group) in enumerate(groups):
        agent = group[0]['agent']
        by = next((r['by'] for r in group if r['variant'] != 'resume'), agent)
        gy = tops[gi]
        # Gridlines and the baseline run under the bars of the group, clear of its title.
        for v in ticks:
            stroke = t['axis'] if v == 0 else t['grid']
            o.append(f'<line x1="{x(v):.1f}" y1="{gy + 20}" x2="{x(v):.1f}" y2="{gy + 26 + len(group) * ROW - (ROW - BAR) + 4}" stroke="{stroke}" stroke-width="1"/>')
        head, detail = header(agent, next((r for r in group if r['variant'] == 'resume'), None))
        if by != agent:
            head = f'{head} session, picked up by {AGENTS[by]}'
        o.append(f'<text x="{PAD}" y="{gy + 14}" font-size="13" font-weight="600" fill="{t["ink"]}">{escape(head)}'
                 f'<tspan font-weight="400" fill="{t["ink2"]}">{escape(detail)}</tspan></text>')
        for ri, r in enumerate(group):
            y = gy + 26 + ri * ROW
            cost = dollars(r['spend'])
            color = t['resume'] if r['variant'] == 'resume' else t['fresh'] if r['by'] == agent else t['other']
            value = r['spend'] + (f" · {r['seconds']} s" if r.get('seconds') else '')
            tip = f"{AGENTS[agent]} session, {LABELS[r['variant']]}" + (f" by {AGENTS[r['by']]}" if r['variant'] != 'resume' and r['by'] != agent else '') + f": {value}" + (f", {r['requests']} requests" if r.get('seconds') else ', forecast')
            o.append(f'<g><title>{escape(tip)}</title>')
            o.append(f'<text x="{x0 - 10}" y="{y + 13}" font-size="12.5" text-anchor="end" fill="{t["ink2"]}">{LABELS[r["variant"]]}</text>')
            o.append(bar(x0, y, max(x(cost) - x0, 2.0), color))
            o.append(f'<text class="num" x="{x(cost) + 8:.1f}" y="{y + 13}" font-size="12" font-weight="600" fill="{t["ink"]}">{escape(value)}</text>')
            o.append('</g>')
    models = sorted({r.get('model') or DEFAULT_MODELS[r['by']] for r in shown if r['variant'] != 'resume'})
    on = 'new sessions on their agent’s default model' if other else 'new sessions on the old session’s model'
    o.append(f'<text x="{PAD}" y="{H - 16}" font-size="11" fill="{t["muted"]}">tokprof bench/reference · one run per variant · {on}: {escape(", ".join(models))}</text>')
    o.append('</svg>')
    return '\n'.join(o) + '\n'


def main():
    args = sys.argv[1:]

    def option(flag):
        if flag not in args:
            return None
        i = args.index(flag)
        value = args[i + 1]
        del args[i:i + 2]
        return value

    specs = option('--sessions')
    title, subtitle = option('--title') or TITLE, option('--subtitle') or SUBTITLE
    out, files = args[0], args[1:]
    rows = [r for f in files for r in json.load(open(f))]
    for r in rows:
        r.setdefault('name', r['agent'])
        r.setdefault('by', r['agent'])
    specs = specs.split(',') if specs else list(dict.fromkeys(r['name'] for r in rows))
    # A run that failed for reasons outside the agent is repeated, and the repeat is charted. A
    # run that saw the prompts of the other runs is left out.
    rows = [r for r in rows if not r.get('failed') and not r.get('contaminated')]
    base, ext = os.path.splitext(out)
    for theme, path in (('light', out), ('dark', f'{base}-dark{ext}')):
        with open(path, 'w') as f:
            f.write(svg(rows, specs, THEMES[theme], title, subtitle))
        print(f'wrote {path}')


if __name__ == '__main__':
    main()
