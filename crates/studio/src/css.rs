/// The stylesheet, served once and cached by the browser.
pub const SHEET: &str = r#"
:root {
  --bg: #0b0f14;
  --surface: #141b23;
  --surface-2: #1b232d;
  --line: #232d38;
  --line-2: #33404e;
  --ink: #e7eef6;
  --muted: #93a3b4;
  --faint: #64748b;

  --side: #0a1512;
  --side-2: #070d0b;
  --side-ink: #cfe6da;
  --side-dim: #6f9186;
  --side-line: #16302a;

  --accent: #00ed64;
  --accent-ink: #00311f;
  --accent-dim: #0a6b40;
  --accent-soft: rgba(0, 237, 100, 0.09);

  --warn: #f5b83d;
  --warn-soft: #2c2413;
  --warn-line: #574620;
  --bad: #ff6b6b;
  --bad-soft: #2a1618;
  --bad-line: #5a2a2d;
  --info: #7db4e6;
  --info-soft: #132234;
  --info-line: #2f4a63;

  --mono: ui-monospace, "Cascadia Code", "JetBrains Mono", Consolas, monospace;
  --ui: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
}

* { box-sizing: border-box; }
html, body { height: 100%; }
body {
  margin: 0; display: flex;
  font-family: var(--ui); font-size: 13.5px; line-height: 1.5;
  color: var(--ink); background: var(--bg);
}
a { color: inherit; text-decoration: none; }
code { font-family: var(--mono); background: var(--surface-2); padding: 1px 6px;
       border-radius: 4px; color: var(--accent); font-size: 0.92em; }

/* ---------- sidebar ---------- */

.side {
  width: 236px; flex-shrink: 0; height: 100vh; position: sticky; top: 0; overflow-y: auto;
  background: linear-gradient(180deg, var(--side), var(--side-2));
  border-right: 1px solid var(--side-line); padding: 16px 10px 28px;
}
.side-head {
  display: flex; align-items: center; gap: 9px; padding: 4px 8px 16px;
  font-weight: 700; font-size: 14px; color: #eafff3;
}
.side-head .dot {
  width: 9px; height: 9px; border-radius: 50%;
  background: var(--accent); box-shadow: 0 0 10px rgba(0, 237, 100, 0.8);
}
.side-sec { padding: 16px 8px 6px; font-size: 10.5px; text-transform: uppercase;
            letter-spacing: 1.4px; color: var(--side-dim); }
.side ul { list-style: none; margin: 0; padding: 0; }
.side li a {
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
  padding: 6px 8px 6px 16px; border-radius: 6px; border-left: 2px solid transparent;
  color: var(--side-ink); transition: background .12s;
}
.side li a:hover { background: rgba(255,255,255,.05); }
.side li a.on { background: var(--accent-soft); border-left-color: var(--accent); color: #eafff3; }
.badge {
  font-family: var(--mono); font-size: 11px; color: var(--side-ink);
  background: rgba(255,255,255,.07); padding: 1px 8px; border-radius: 9px;
  min-width: 24px; text-align: center;
}
.side li a.on .badge { background: var(--accent); color: var(--accent-ink); font-weight: 700; }
.badge.hot { color: var(--bad); }
.side form { padding: 8px 8px 0; }
.side input {
  width: 100%; background: rgba(0,0,0,.35); border: 1px solid var(--side-line);
  color: var(--side-ink); padding: 7px 11px; border-radius: 7px; font: inherit;
}
.side input::placeholder { color: var(--side-dim); }
.side input:focus { outline: none; border-color: var(--accent-dim); }

/* ---------- main ---------- */

.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.bar {
  position: sticky; top: 0; z-index: 5;
  display: flex; align-items: center; justify-content: space-between; gap: 16px;
  padding: 13px 24px; background: rgba(11,15,20,.88); backdrop-filter: blur(8px);
  border-bottom: 1px solid var(--line);
}
.crumb { font-size: 13px; color: var(--muted); display: flex; align-items: center;
         gap: 7px; flex-wrap: wrap; }
.crumb a:hover { color: var(--accent); }
.crumb .sep { color: var(--faint); }
.crumb .cur { color: var(--ink); font-weight: 600; }
.chip {
  font-family: var(--mono); font-size: 11.5px; color: var(--accent);
  background: var(--accent-soft); border: 1px solid var(--accent-dim);
  padding: 2px 11px; border-radius: 20px; white-space: nowrap;
}
.chip.hot { color: var(--bad); background: var(--bad-soft); border-color: var(--bad-line); }
.chip.zero { color: var(--muted); background: var(--surface-2); border-color: var(--line); }
.wrap { padding: 20px 24px 56px; max-width: 1120px; }

h1 { font-size: 26px; font-weight: 750; letter-spacing: -.4px; margin: 0 0 8px; }
h2 { font-size: 11px; text-transform: uppercase; letter-spacing: 1.6px;
     color: var(--muted); margin: 28px 0 12px; font-weight: 600; }
p.why { color: var(--muted); font-size: 12.5px; margin: 0 0 12px; }
p.note { color: var(--faint); font-size: 11.5px; margin: 10px 0 8px; }
.empty { color: var(--muted); padding: 24px; background: var(--surface);
         border: 1px dashed var(--line-2); border-radius: 10px; }
.pager { display: flex; align-items: center; justify-content: space-between; gap: 12px;
         margin: 14px 0 4px; font-size: 12.5px; }
.pager a { color: var(--accent); padding: 5px 12px; border: 1px solid var(--accent-dim);
           border-radius: 7px; background: var(--accent-soft); }
.pager a:hover { border-color: var(--accent); }
.pager .off { color: var(--faint); padding: 5px 12px; border: 1px solid var(--line);
              border-radius: 7px; }
.pager .range { color: var(--muted); font-family: var(--mono); font-size: 11.5px; }

/* ---------- chips: what a screen is narrowed to ---------- */

.chips { display: flex; flex-wrap: wrap; gap: 6px; margin: 0 0 10px; }
.chips.sub { margin-top: -4px; }
.pick {
  display: inline-flex; align-items: baseline; gap: 6px;
  padding: 4px 11px; border-radius: 8px; font-size: 12.5px;
  background: var(--surface); border: 1px solid var(--line); color: var(--muted);
}
.pick:hover { border-color: var(--line-2); color: var(--ink); }
.pick.on { background: var(--accent-soft); border-color: var(--accent-dim); color: #eafff3; }
.pick .num { font-family: var(--mono); font-size: 11px; color: var(--faint); }
.pick.on .num { color: var(--accent); }
.pick .num.bad { color: var(--bad); }
.chips.sub .pick { font-size: 12px; padding: 3px 9px; }

/* ---------- filter bar ---------- */

.filter { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin: 0 0 12px; }
.filter input[type=search] {
  font: inherit; font-size: 12.5px; padding: 5px 11px; border-radius: 7px; min-width: 260px;
  background: var(--bg); border: 1px solid var(--line-2); color: var(--ink);
}
.filter input[type=search]:focus { outline: none; border-color: var(--accent-dim); }
.filter select {
  font: inherit; font-size: 12.5px; padding: 5px 9px; border-radius: 7px;
  background: var(--bg); border: 1px solid var(--line-2); color: var(--ink);
}
.filter a.plain { color: var(--muted); font-size: 12px; padding: 5px 8px; }
.filter a.plain:hover { color: var(--accent); }

/* rows a person edits in place */
table.rowset td { vertical-align: middle; }
/* The field a name is written into takes the room it needs: half the row, filled edge to edge,
   with the badge that says where the current name came from beside it. */
.rowset td.ru { width: 46%; }
.rowset .ru-cell { display: flex; align-items: center; gap: 8px; }
.rowset .ru-cell form.inline { flex: 1; display: flex; min-width: 0; }
.rowset .ru-cell input[type=text] { flex: 1; min-width: 0; }
.rowset .ru-cell .tag, .rowset .ru-cell .prov { flex: 0 0 auto; }
/* Big enough to recognise the artwork at a glance, and portrait so a mod card is not cropped. */
.rowset .iref { gap: 12px; }
.rowset .iref-icon { width: 64px; height: 84px; border-radius: 8px; padding: 2px; }
.rowset .iref-ru { font-size: 14px; }
.rowset .iref-en { font-size: 11.5px; }

/* ---------- sidebar footer ---------- */

.side-build { padding: 4px 8px; }
.side-build button { width: 100%; }
button.warn { border-color: var(--warn-line); background: var(--warn-soft); color: var(--warn); }
button.warn:hover { background: var(--warn); color: #241a05; }
.side-note { color: var(--side-dim); font-size: 11px; padding: 6px 10px 0; margin: 0; }

/* ---------- stat grids ---------- */

.stats { display: grid; grid-template-columns: repeat(auto-fill, minmax(146px, 1fr));
         gap: 10px; margin-bottom: 10px; }
.stat { display: flex; flex-direction: column; gap: 2px; padding: 13px 15px;
        border-radius: 10px; background: var(--surface); border: 1px solid var(--line); }
a.stat:hover { border-color: var(--accent); }
.stat .n { font-family: var(--mono); font-size: 22px; font-weight: 700; }
.stat .l { font-size: 11.5px; color: var(--muted); }
.stat.ok { border-color: var(--accent-dim); } .stat.ok .n { color: var(--accent); }
.stat.bad { border-color: var(--bad-line); background: var(--bad-soft); }
.stat.bad .n { color: var(--bad); }
.stat.warn { border-color: var(--warn-line); } .stat.warn .n { color: var(--warn); }

/* ---------- cards ---------- */

.card { background: var(--surface); border: 1px solid var(--line); border-radius: 11px;
        padding: 14px 18px; margin-bottom: 12px; }
.card-h { display: flex; align-items: center; justify-content: space-between;
          gap: 12px; margin-bottom: 10px; }
.card-t { font-weight: 650; font-size: 13.5px; }
.card-n { font-family: var(--mono); font-size: 11.5px; color: var(--muted);
          background: var(--surface-2); border: 1px solid var(--line);
          border-radius: 20px; padding: 1px 10px; }
.card-n.hot { color: var(--bad); background: var(--bad-soft); border-color: var(--bad-line); }
.card-n.zero { color: var(--accent); background: transparent; }
.cols { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: start; }
@media (max-width: 900px) { .cols { grid-template-columns: 1fr; } }

/* ---------- tags ---------- */

.tags { display: flex; flex-wrap: wrap; gap: 6px; margin: 8px 0; }
.tag { font-size: 11.5px; padding: 2px 9px; border-radius: 6px; background: var(--surface-2);
       border: 1px solid var(--line); color: var(--muted); }
.tag.kind { color: var(--ink); border-color: var(--line-2); }
.tag.prime { color: var(--warn); border-color: var(--warn-line); background: var(--warn-soft); }
.tag.trade { color: var(--accent); border-color: var(--accent-dim); background: var(--accent-soft); }
.tag.bad { color: var(--bad); border-color: var(--bad-line); background: var(--bad-soft); }
.tag.alone { color: var(--warn); border-color: var(--warn-line); background: var(--warn-soft); }
.path { font-family: var(--mono); font-size: 10.5px; color: var(--faint); word-break: break-all; }

/* ---------- provenance ---------- */

.prov { font-family: var(--mono); font-size: 9px; font-weight: 700; letter-spacing: .3px;
        text-transform: uppercase; padding: 1px 5px; border-radius: 4px; border: 1px solid;
        white-space: nowrap; }
.prov.de { color: var(--accent); border-color: var(--accent-dim); background: var(--accent-soft); }
.prov.wfm { color: var(--info); border-color: var(--info-line); background: var(--info-soft); }
.prov.rule { color: var(--muted); border-color: var(--line-2); background: var(--surface-2); }
.prov.curated { color: var(--warn); border-color: var(--warn-line); background: var(--warn-soft); }
.prov.wiki { color: var(--info); border-color: var(--info-line); background: var(--info-soft); }

/* ---------- tables ---------- */

table { width: 100%; border-collapse: collapse; font-size: 12.5px; }
thead th { text-align: left; font-weight: 600; font-size: 10.5px; text-transform: uppercase;
           letter-spacing: .8px; color: var(--faint); padding: 4px 10px 6px;
           border-bottom: 1px solid var(--line); }
td { padding: 6px 10px; border-bottom: 1px solid var(--line); vertical-align: top; }
tr:last-child td { border-bottom: 0; }
tbody tr:hover td { background: rgba(255,255,255,.02); }
td.num { font-family: var(--mono); text-align: right; white-space: nowrap; }
td.dim { color: var(--muted); }
.scroll { overflow-x: auto; }

/* A value in Russian with the source's own wording under it. */
.bi { display: inline-flex; flex-direction: column; line-height: 1.25; min-width: 0; }
.bi-en { font-size: 10.5px; color: var(--faint); }
a:hover .bi-ru { color: var(--accent); }
thead th .col-en { display: block; margin-top: 1px; font-size: 9px; font-weight: 400;
                   letter-spacing: .4px; text-transform: none; color: var(--line-2); }

/* Who holds a star-chart node, with the emblem the game draws for them. */
.target { display: inline-flex; align-items: center; gap: 8px; }
.target-icon { width: 24px; height: 24px; flex: 0 0 auto; object-fit: contain; }

/* ---------- folded lists ---------- */
/* Every row stays in the page; the toggle only hides the tail, so find-in-page still works. */

.fold:has(> details:not([open])) .folded { display: none; }
.fold > details { margin-top: 9px; }
.fold > details > summary {
  display: inline-flex; align-items: center; gap: 6px; width: fit-content; cursor: pointer;
  list-style: none; font-size: 12px; padding: 4px 12px; border-radius: 7px;
  color: var(--accent); background: var(--accent-soft); border: 1px solid var(--accent-dim);
}
.fold > details > summary::-webkit-details-marker { display: none; }
.fold > details > summary:hover { border-color: var(--accent); }
.fold > details[open] > summary .fold-more { display: none; }
.fold > details:not([open]) > summary .fold-less { display: none; }

.iref { display: inline-flex; align-items: center; gap: 9px; }
.iref-icon { width: 36px; height: 36px; flex-shrink: 0; border-radius: 6px;
             background: var(--surface-2); border: 1px solid var(--line); object-fit: contain; }
.iref-text { display: flex; flex-direction: column; line-height: 1.2; min-width: 0; }
.iref-ru { font-weight: 600; }
.iref-en { font-size: 11px; color: var(--faint); }
.iref:hover .iref-ru { color: var(--accent); }

/* ---------- entity head ---------- */

/* Boxes are portrait: a mod's picture is its whole card, taller than it is wide, while
   every other icon is square and simply centres inside. */

.head { display: flex; gap: 18px; align-items: flex-start; margin-bottom: 18px; }
.shot { width: 108px; height: 140px; flex-shrink: 0; border-radius: 12px;
        background: var(--surface-2); border: 1px solid var(--line);
        display: flex; align-items: center; justify-content: center; overflow: hidden; }
.shot img { max-width: 100%; max-height: 100%; object-fit: contain; }
.shot .none { font-family: var(--mono); font-size: 10px; color: var(--faint); text-align: center; }
.head .grow { min-width: 0; flex: 1; }
.ru { font-size: 15px; color: var(--muted); margin: 0 0 6px; }
.ru.none { color: var(--bad); }

.shots { display: flex; flex-wrap: wrap; gap: 18px; }
.pic { display: flex; gap: 12px; align-items: flex-start; }
.pic-box {
  width: 132px; height: 172px; flex-shrink: 0; border-radius: 9px;
  background: var(--surface-2); border: 1px solid var(--line);
  display: flex; align-items: center; justify-content: center; overflow: hidden;
}
.pic-box img { max-width: 100%; max-height: 100%; object-fit: contain; }
.pic-facts { display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
.pic-facts .tags { margin: 0; }

/* ---------- craft strip ---------- */

.strip { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 10px; }
.strip .arrow { align-self: center; font-family: var(--mono); font-size: 18px;
                color: var(--accent); padding: 0 2px; }
.ctile { display: flex; flex-direction: column; align-items: center; gap: 5px;
         width: 92px; text-align: center; }
.ctile-box { width: 84px; height: 108px; border-radius: 11px; background: var(--surface-2);
             border: 1px solid var(--line); display: flex; align-items: center;
             justify-content: center; overflow: hidden; transition: border-color .12s, transform .12s; }
.ctile:hover .ctile-box { border-color: var(--accent); transform: translateY(-2px); }
.ctile.out .ctile-box { border-color: var(--accent-dim); box-shadow: inset 0 0 0 1px var(--accent-dim); }
.ctile-box img { max-width: 100%; max-height: 100%; object-fit: contain; }
.ctile-box .ph { font-family: var(--mono); font-size: 9px; color: var(--faint);
                 padding: 4px; line-height: 1.2; word-break: break-word; }
.ctile .c { font-family: var(--mono); font-size: 12.5px; font-weight: 700; color: var(--accent); }
.ctile .r { font-size: 11px; color: var(--muted); max-width: 78px; overflow: hidden;
            text-overflow: ellipsis; white-space: nowrap; }
.meta { display: flex; flex-wrap: wrap; gap: 20px; margin-top: 14px; padding-top: 12px;
        border-top: 1px solid var(--line); font-size: 12.5px; color: var(--muted); }
.meta b { font-family: var(--mono); color: var(--ink); margin-left: 7px; }

/* ---------- forms ---------- */

button {
  font: inherit; font-size: 12px; padding: 5px 14px; border-radius: 7px; cursor: pointer;
  border: 1px solid var(--accent-dim); background: var(--accent-soft); color: var(--accent);
}
button:hover { background: var(--accent); color: var(--accent-ink); }
button.plain { border-color: var(--line-2); background: var(--surface-2); color: var(--muted); }
button.plain:hover { background: var(--line); color: var(--ink); }
form.inline { display: inline-flex; gap: 6px; align-items: center; margin: 0; }
.curate-row { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin-bottom: 10px; }
.curate-row > .dim { min-width: 110px; }
input[type=text], input[type=search] {
  font: inherit; font-size: 12.5px; padding: 5px 10px; border-radius: 7px;
  background: var(--bg); border: 1px solid var(--line-2); color: var(--ink); min-width: 190px;
}
input[type=text]::placeholder, input[type=search]::placeholder { color: var(--faint); }
input[type=text]:focus, input[type=search]:focus { outline: none; border-color: var(--accent-dim); }
.curate-row input[type=search] { flex: 1; min-width: 220px; }

/* ---------- queue rows ---------- */

.rows { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.row { background: var(--surface); border: 1px solid var(--line); border-radius: 10px;
       padding: 11px 15px; }
.row:hover { border-color: var(--line-2); }
.row-h { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
.row-t { font-weight: 600; font-size: 13.5px; }
/* What a row is about, told apart from the thing it is about by size and case. */
.eyebrow { font-size: 10.5px; font-weight: 600; text-transform: uppercase;
           letter-spacing: 1.1px; color: var(--muted); }
.subject .iref-icon { width: 44px; height: 56px; }
.subject .iref-ru { font-size: 14.5px; }
.subject .row-t { font-size: 14.5px; }
.opts { display: flex; flex-direction: column; gap: 5px; margin-top: 9px;
        padding-top: 9px; border-top: 1px solid var(--line); }
.opt { display: flex; align-items: center; gap: 10px; justify-content: space-between; }
.opt .grow { min-width: 0; flex: 1; }
.score { font-family: var(--mono); font-size: 10.5px; color: var(--muted);
         border: 1px solid var(--line); border-radius: 20px; padding: 0 8px; }
.score.sure { color: var(--accent); border-color: var(--accent-dim); }
/* The other verdict: the name belongs to nothing the catalog can hold. Sits apart from the
   candidates so it is not read as one of them. */
.curate-row.verdict { margin: 9px 0 0; padding-top: 9px; border-top: 1px solid var(--line); }
.curate-row.verdict form.inline { flex: 1; }
.curate-row.verdict input[type=text] { flex: 1; min-width: 220px; }

/* ---------- conflict cards ---------- */

.mini { display: flex; gap: 13px; align-items: flex-start; }
.mini-shot {
  width: 68px; height: 88px; flex-shrink: 0; border-radius: 9px;
  background: var(--surface-2); border: 1px solid var(--line); overflow: hidden;
  display: flex; align-items: center; justify-content: center;
}
.mini-shot img { max-width: 100%; max-height: 100%; object-fit: contain; display: block; }
.mini .tags { margin: 5px 0 3px; }

.cmp-h {
  display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap;
  margin: 12px 0 6px; padding-top: 10px; border-top: 1px solid var(--line);
}
.cmp-prop {
  font-size: 10.5px; text-transform: uppercase; letter-spacing: 1.2px;
  color: var(--ink); font-weight: 600;
}
.cmp-kept { font-family: var(--mono); font-size: 11.5px; color: var(--accent); }

.cmp {
  display: grid; grid-auto-flow: column; grid-auto-columns: minmax(190px, 1fr);
  gap: 10px; overflow-x: auto; padding-bottom: 2px;
}
.col {
  display: flex; flex-direction: column; gap: 6px; align-items: flex-start;
  background: var(--surface-2); border: 1px solid var(--line);
  border-radius: 9px; padding: 9px 11px; min-width: 0;
}
.col.kept { border-color: var(--accent-dim); background: var(--accent-soft); }
.col-h { display: flex; align-items: center; gap: 6px; font-size: 11px; }
.col-v { font-size: 13.5px; overflow-wrap: anywhere; }
.diff { color: var(--warn); background: var(--warn-soft); border-radius: 3px; padding: 0 2px; }
/* On a candidate's name whole words get marked, so the mark has to be quiet enough to read
   through. */
.iref-en .diff { background: none; padding: 0; text-decoration: underline dotted;
                 text-underline-offset: 2px; }
.col input[type=text] { min-width: 0; width: 100%; }
@media (max-width: 700px) { .cmp { grid-auto-flow: row; } }

.done { background: var(--accent-soft); border: 1px solid var(--accent-dim);
        border-radius: 10px; padding: 10px 15px; margin-bottom: 14px;
        display: flex; align-items: center; justify-content: space-between; gap: 12px; }
/* a settled row keeps the row layout and only changes colour */
.row.done { display: block; margin-bottom: 0; border-color: var(--accent-dim); }

/* ---------- relation graph ---------- */

.gwrap { margin: 2px 0 4px; }
.gedges { position: absolute; inset: 0; overflow: visible; pointer-events: none; }
.gedge { fill: none; stroke: var(--line-2); stroke-width: 1.5; }
.gnode { position: absolute; display: flex; align-items: center; gap: 9px; padding: 0 11px;
         background: var(--surface); border: 1px solid var(--line); border-radius: 11px; }
a.gnode:hover { border-color: var(--accent); background: var(--surface-2); }
.gnode.cur { border-color: var(--accent); background: var(--accent-soft);
             box-shadow: 0 0 0 1px var(--accent-dim), 0 0 18px rgba(0, 237, 100, 0.22); }
.gnode.more { justify-content: center; color: var(--muted); border-style: dashed; }
.gicon { width: 36px; height: 36px; flex: 0 0 auto; object-fit: contain; border-radius: 7px;
         background: var(--surface-2); }
.gtext { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.grel { font-size: 9px; text-transform: uppercase; letter-spacing: 0.6px; color: var(--faint); }
.gname { font-size: 12.5px; line-height: 1.2; color: var(--ink); overflow: hidden;
         display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
.gnode.cur .gname { color: #eafff3; font-weight: 600; }

/* ---------- responsive ---------- */

@media (max-width: 760px) {
  body { flex-direction: column; }
  .side { width: 100%; height: auto; position: static; }
  .head { flex-direction: column; }
}
:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) { * { transition: none !important; } }
"#;
