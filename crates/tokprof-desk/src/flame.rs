//! The `Flame` tab: the selected session's spend as an icicle graph, the session on top and each
//! frame's children below it, from the stacks `tokprof flame --session <id>` writes. The tree is
//! built once per X-ray; a hover, a key or a zoom only lays it out again.

use std::collections::HashMap;
use std::rc::Rc;

use rune::{Color, Cx, El, KeyEvent, col, el, row, spacer, text};
use tokprof_core::fmt::{tokens, usd};
use tokprof_core::profile::{Measure, Stacks, Tint};
use tokprof_core::xray::XRay;

use crate::Desk;
use crate::overlay::{self, PILL, R_GROUP, R_MARK, R_ROW};
use crate::pal::{self, Pal};

/// The key of the graph, whose width the bars are laid out in.
pub const GRAPH: &str = "flame-graph";
/// The height of a depth, and the gap between bars and between depths, in points.
pub const ROW: f32 = 22.0;
pub const GAP: f32 = 2.0;
/// Frames narrower than this are not drawn.
pub const MIN_W: f32 = 3.0;
/// A bar this wide or wider gets a label.
const LABEL_W: f32 = 28.0;
/// The width the graph is laid out in before its first layout.
const GUESS_W: f32 = 640.0;
/// The leading dimensions that fold into the root when every stack shares them.
const HEAD: [&str; 3] = ["agent", "project", "session"];

/// A frame of the tree: its name and kind, its value (µ$, or tokens without prices), and where
/// it sits.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub tint: Tint,
    pub value: i64,
    pub parent: Option<usize>,
    /// Largest first, then by name.
    pub children: Vec<usize>,
    pub depth: usize,
    /// This frame and every frame below it.
    pub size: usize,
}

/// A step of a path from the root: a frame's name and kind.
pub type Step = (String, Tint);

/// The frame tree of a session, node 0 its root.
#[derive(Clone, Debug)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// The frames every stack started with, folded into the root.
    pub head: Vec<String>,
    /// The values are tokens: no request of the session had a price.
    pub in_tokens: bool,
}

impl Tree {
    /// The tree of these stacks: each stack's value as `.collapsed` writes it (µ$ rounded per
    /// stack, or tokens), stacks of 0 left out, frames merged by name and kind.
    pub fn of(stacks: &Stacks, priced: bool) -> Tree {
        let measure = if priced { Measure::Cost } else { Measure::Tokens };
        let kept: Vec<_> = stacks.iter().map(|(s, v)| (s, measure.of(v))).filter(|(s, v)| *v > 0 && !s.is_empty()).collect();
        // The head every stack shares folds into the root.
        let mut fold = 0;
        while fold < HEAD.len()
            && let Some((first, _)) = kept.first()
            && first.get(fold).is_some_and(|f| f.dim == HEAD[fold] && f.tint == Tint::Structure)
            && kept.iter().all(|(s, _)| s.len() > fold + 1 && s[fold] == first[fold])
        {
            fold += 1;
        }
        let head: Vec<String> = kept.first().map(|(s, _)| s[..fold].iter().map(|f| f.name.clone()).collect()).unwrap_or_default();
        let root = Node { name: String::new(), tint: Tint::Structure, value: 0, parent: None, children: Vec::new(), depth: 0, size: 1 };
        let mut nodes = vec![root];
        let mut index: HashMap<(usize, &str, Tint), usize> = HashMap::new();
        for (stack, value) in &kept {
            let mut at = 0;
            nodes[0].value += value;
            for f in &stack[fold..] {
                let name = if f.name.is_empty() { "(empty)" } else { f.name.as_str() };
                at = *index.entry((at, name, f.tint)).or_insert_with(|| {
                    let depth = nodes[at].depth + 1;
                    nodes.push(Node { name: name.to_string(), tint: f.tint, value: 0, parent: Some(at), children: Vec::new(), depth, size: 1 });
                    let i = nodes.len() - 1;
                    nodes[at].children.push(i);
                    i
                });
                nodes[at].value += value;
            }
        }
        for i in 0..nodes.len() {
            let mut children = std::mem::take(&mut nodes[i].children);
            children.sort_by(|&a, &b| nodes[b].value.cmp(&nodes[a].value).then_with(|| nodes[a].name.cmp(&nodes[b].name)));
            nodes[i].children = children;
        }
        // Children come after their parents, so sizes sum up from the end.
        for i in (1..nodes.len()).rev() {
            let (size, parent) = (nodes[i].size, nodes[i].parent.unwrap_or(0));
            nodes[parent].size += size;
        }
        nodes[0].name = if head.is_empty() { "all".to_string() } else { head.join(" · ") };
        Tree { nodes, head, in_tokens: !priced }
    }

    pub fn total(&self) -> i64 {
        self.nodes[0].value
    }

    /// A value as the tab shows it: dollars, or tokens.
    pub fn amount(&self, v: i64) -> String {
        if self.in_tokens { format!("{} tok", tokens(v as f64)) } else { usd(v as f64 / 1e6) }
    }

    /// The steps from the root to frame `i`.
    pub fn path(&self, mut i: usize) -> Vec<Step> {
        let mut out = Vec::new();
        while let Some(parent) = self.nodes[i].parent {
            out.push((self.nodes[i].name.clone(), self.nodes[i].tint));
            i = parent;
        }
        out.reverse();
        out
    }

    /// The frame at the end of these steps.
    pub fn find(&self, path: &[Step]) -> Option<usize> {
        path.iter()
            .try_fold(0, |at, (name, tint)| self.nodes[at].children.iter().copied().find(|&c| self.nodes[c].name == *name && self.nodes[c].tint == *tint))
    }

    /// Frame `i` and the frames above it, the root first.
    pub fn line(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        while let Some(parent) = self.nodes[out[out.len() - 1]].parent {
            out.push(parent);
        }
        out.reverse();
        out
    }

    /// Whether frame `i` is `root` or below it.
    pub fn within(&self, i: usize, root: usize) -> bool {
        self.line(i).contains(&root)
    }

    /// The full path of frame `i`, the folded head included.
    pub fn names(&self, i: usize) -> String {
        let mut names: Vec<&str> = self.head.iter().map(String::as_str).collect();
        names.extend(self.line(i).into_iter().skip(1).map(|n| self.nodes[n].name.as_str()));
        if names.is_empty() { "all".to_string() } else { names.join(" › ") }
    }

    /// The width of frame `i`'s bar when `root` is shown in `width` points: its share of the root
    /// over the width and one gap, less the gap.
    pub fn bar_w(&self, i: usize, root: usize, width: f32) -> f32 {
        let total = self.nodes[root].value.max(1) as f64;
        (self.nodes[i].value as f64 / total * (width + GAP) as f64) as f32 - GAP
    }

    /// Whether frame `i` is drawn when `root` is shown in `width` points.
    pub fn drawn(&self, i: usize, root: usize, width: f32) -> bool {
        self.bar_w(i, root, width) >= MIN_W
    }
}

/// A bar of the graph: its frame, its place in the graph and its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub node: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

/// The bars of `root` shown in `width` points, and how many frames under it are too narrow to
/// draw, their children included.
pub fn layout(tree: &Tree, root: usize, width: f32) -> (Vec<Bar>, usize) {
    let scale = (width + GAP) as f64 / tree.nodes[root].value.max(1) as f64;
    let (mut bars, mut hidden) = (Vec::new(), 0);
    let base = tree.nodes[root].depth;
    let mut stack = vec![(root, 0.0f64)];
    while let Some((i, x)) = stack.pop() {
        let n = &tree.nodes[i];
        let w = (n.value as f64 * scale) as f32 - GAP;
        if w < MIN_W {
            hidden += n.size;
            continue;
        }
        bars.push(Bar { node: i, x: x as f32, y: (n.depth - base) as f32 * (ROW + GAP), w });
        let mut at = x;
        let mut next = Vec::new();
        for &c in &n.children {
            next.push((c, at));
            at += tree.nodes[c].value as f64 * scale;
        }
        stack.extend(next.into_iter().rev());
    }
    (bars, hidden)
}

/// The tab's state: the shown root, the cursor and the frame under the pointer, as paths of the
/// session they were set in, so that a new X-ray of the same session keeps them.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct State {
    pub session: String,
    pub root: Vec<Step>,
    pub cursor: Vec<Step>,
    pub hover: Option<Vec<Step>>,
    /// Zoom this many levels into the largest frames once the tree is there (for a scene).
    pub dive: u8,
}

/// The tree of an X-ray, and the X-ray it was built from.
#[derive(Default)]
pub struct Memo(Option<(usize, String, u64, Rc<Tree>)>);

/// How a key moves the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Before,
    After,
    Child,
    Parent,
}

/// Where a move takes the cursor from `at`, through the frames drawn when `root` is shown in
/// `width` points; `None` when it cannot go there.
pub fn step(tree: &Tree, root: usize, width: f32, at: usize, m: Move) -> Option<usize> {
    let drawn = |i: usize| tree.drawn(i, root, width);
    match m {
        Move::Parent => (at != root).then(|| tree.nodes[at].parent).flatten(),
        Move::Child => tree.nodes[at].children.first().copied().filter(|&c| drawn(c)),
        Move::Before | Move::After if at == root => None,
        Move::Before | Move::After => {
            let siblings: Vec<usize> = tree.nodes[tree.nodes[at].parent?].children.iter().copied().filter(|&c| drawn(c)).collect();
            let k = siblings.iter().position(|&c| c == at)?;
            if m == Move::Before { k.checked_sub(1).map(|k| siblings[k]) } else { siblings.get(k + 1).copied() }
        }
    }
}

impl Desk {
    /// The frame tree of the X-ray shown, built once per X-ray.
    pub fn flame_tree(&self) -> Option<Rc<Tree>> {
        let x = self.browser.xray()?;
        let id = x as *const XRay as usize;
        let mut memo = self.flame_memo.borrow_mut();
        if let Some((at, session, requests, tree)) = &memo.0
            && (*at, session.as_str(), *requests) == (id, x.id.as_str(), x.requests)
        {
            return Some(tree.clone());
        }
        let tree = Rc::new(Tree::of(&x.stacks, x.priced));
        memo.0 = Some((id, x.id.clone(), x.requests, tree.clone()));
        Some(tree)
    }

    /// The shown root and the cursor in this tree: the root for another session, or for a path
    /// it no longer has.
    pub fn flame_at(&self, tree: &Tree) -> (usize, usize) {
        let Some(x) = self.browser.xray() else { return (0, 0) };
        if self.flame.session != x.id {
            return (0, 0);
        }
        let root = tree.find(&self.flame.root).unwrap_or(0);
        let cursor = tree.find(&self.flame.cursor).filter(|&c| tree.within(c, root)).unwrap_or(root);
        (root, cursor)
    }

    /// The frame under the pointer, in this tree.
    fn flame_hover(&self, tree: &Tree) -> Option<usize> {
        let x = self.browser.xray()?;
        (self.flame.session == x.id).then(|| self.flame.hover.as_ref().and_then(|h| tree.find(h))).flatten()
    }

    /// Shows `root` with the cursor on `cursor`.
    fn flame_set(&mut self, tree: &Tree, root: usize, cursor: usize) {
        let session = self.browser.xray().map(|x| x.id.clone()).unwrap_or_default();
        if self.flame.session != session {
            self.flame = State { session, ..State::default() };
        }
        self.flame.root = tree.path(root);
        self.flame.cursor = tree.path(cursor);
    }

    /// A click on frame `i`: it becomes the root, or, when it is the root, its parent does.
    pub fn flame_click(&mut self, i: usize) {
        let Some(tree) = self.flame_tree() else { return };
        let (root, _) = self.flame_at(&tree);
        self.pane = crate::Focus::Inspector;
        match tree.nodes[i].parent {
            Some(parent) if i == root => self.flame_set(&tree, parent, i),
            _ => self.flame_set(&tree, i, i),
        }
    }

    /// A click on a crumb: frame `i` above the shown root becomes the root, with the cursor where
    /// the root was.
    pub fn flame_crumb(&mut self, i: usize) {
        let Some(tree) = self.flame_tree() else { return };
        let (root, _) = self.flame_at(&tree);
        self.pane = crate::Focus::Inspector;
        self.flame_set(&tree, i, root);
    }

    /// Zooms out one level; false at the top.
    pub fn flame_out(&mut self) -> bool {
        let Some(tree) = self.flame_tree() else { return false };
        let (root, _) = self.flame_at(&tree);
        let Some(parent) = tree.nodes[root].parent else { return false };
        self.flame_set(&tree, parent, root);
        true
    }

    /// The keys of the graph while the inspector has the keyboard on `Flame`; Shift+← and → are
    /// left to switch tabs.
    pub fn flame_key(&mut self, e: &KeyEvent) -> bool {
        use rune::Key as K;
        let Some(tree) = self.flame_tree() else { return false };
        let plain = !(e.mods.cmd || e.mods.ctrl || e.mods.alt);
        let text = match &e.key {
            K::Text(t) if plain && !e.mods.shift => Some(t.as_str()),
            _ => None,
        };
        let m = match (&e.key, text) {
            (K::Left, _) | (_, Some("h")) if !e.mods.shift => Move::Before,
            (K::Right, _) | (_, Some("l")) if !e.mods.shift => Move::After,
            (K::Down, _) | (_, Some("j")) => Move::Child,
            (K::Up, _) | (_, Some("k")) => Move::Parent,
            (K::Enter, _) => {
                let (_, cursor) = self.flame_at(&tree);
                self.flame_set(&tree, cursor, cursor);
                return true;
            }
            // At the top there is nothing to zoom out of: ⌫ goes on, and leaves full size.
            (K::Backspace, _) => return self.flame_out(),
            _ => return false,
        };
        let (root, cursor) = self.flame_at(&tree);
        if let Some(to) = step(&tree, root, self.flame_width(), cursor, m) {
            self.flame_set(&tree, root, to);
        }
        true
    }

    /// The width the graph was last laid out in.
    pub fn flame_width(&self) -> f32 {
        if self.flame_w > 0.0 { self.flame_w } else { GUESS_W }
    }

    /// For a scene: zooms into the largest frames, as many levels as asked, once the tree is
    /// there; true when it did.
    pub fn flame_dive(&mut self) -> bool {
        if self.flame.dive == 0 {
            return false;
        }
        let Some(tree) = self.flame_tree() else { return false };
        let mut at = 0;
        for _ in 0..self.flame.dive {
            at = tree.nodes[at].children.first().copied().unwrap_or(at);
        }
        self.flame.dive = 0;
        self.flame_set(&tree, at, at);
        true
    }
}

/// What the tab shows, for the inspector's `memo`.
pub fn version(s: &Desk, cx: &Cx<Desk>) -> u64 {
    crate::view::hash_of((&s.flame, cx.layout_size(GRAPH).map(|sz| sz[0].to_bits())))
}

/// The color of a frame by what it stands for, and how much of it tints the bar.
pub fn color(tint: Tint, p: &Pal) -> (Color, f32) {
    match tint {
        Tint::Structure => (p.neutral, 0.2),
        Tint::Content => (p.work, 0.5),
        Tint::Read => (p.read, 0.36),
        Tint::Write => (p.write, 0.4),
        Tint::Miss => (p.cold, 0.34),
        Tint::Output => (p.out, 0.36),
        Tint::Uncached => (p.question, 0.32),
    }
}

/// The fill of a frame's bar: its color mixed into the group behind the graph.
pub fn fill(tint: Tint, p: &Pal) -> Color {
    let (c, share) = color(tint, p);
    // The mix is in linear light, where a share of a bright color reads much louder on a dark
    // group than on a light one.
    let share = if p.bar.luminance() < 0.3 { share * 0.3 } else { share };
    p.bar.mix(c.alpha(1.0), share)
}

/// The line under the graph about frame `i`: its path, its value, its share of the session and
/// of its parent.
pub fn describe(tree: &Tree, i: usize) -> String {
    let n = &tree.nodes[i];
    let share = |of: i64| 100.0 * n.value as f64 / of.max(1) as f64;
    let mut out = format!("{} · {} · {:.1}% of the session", tree.names(i), tree.amount(n.value), share(tree.total()));
    if let Some(parent) = n.parent.filter(|&p| p != 0) {
        out += &format!(" · {:.1}% of {}", share(tree.nodes[parent].value), tree.nodes[parent].name);
    }
    out
}

/// The note on the hidden frames, empty without any.
pub fn hidden_note(hidden: usize) -> String {
    match hidden {
        0 => String::new(),
        1 => "1 frame under 3 pt hidden".to_string(),
        n => format!("{n} frames under 3 pt hidden"),
    }
}

/// The tab's blocks: the breadcrumb, the graph and the lines under it.
pub fn tab(s: &Desk, x: &XRay, cx: &Cx<Desk>, p: &Pal) -> Vec<El> {
    let Some(tree) = s.flame_tree() else { return Vec::new() };
    if tree.total() <= 0 {
        return vec![text(format!("{} has no spend to draw", x.short_id)).font(pal::BODY).color(p.ink2)];
    }
    let width = cx.layout_size(GRAPH).map(|sz| sz[0]).filter(|w| *w > 0.0).unwrap_or(GUESS_W);
    let (root, cursor) = s.flame_at(&tree);
    let ring = crate::inspector::keyboard(s);
    let (bars, hidden) = layout(&tree, root, width);
    let depth = bars.iter().map(|b| tree.nodes[b.node].depth - tree.nodes[root].depth).max().unwrap_or(0);
    let graph = el()
        .key(GRAPH)
        .w_full()
        .h((depth + 1) as f32 * (ROW + GAP) - GAP)
        .on_layout(|s: &mut Desk, r| s.flame_w = r[2])
        .on_hover_change(|s: &mut Desk, inside, _| {
            if !inside {
                s.flame.hover = None;
            }
        })
        .children(bars.iter().map(|b| bar(&tree, b, ring && b.node == cursor, p)));
    let group = col().key("flame-group").w_full().padding(12.0).radius(R_GROUP).fill(p.bar).child(graph);
    let shown = s.flame_hover(&tree).unwrap_or(cursor);
    let info = row()
        .key("flame-info")
        .w_full()
        .gap(12.0)
        .center_y()
        .child(text(describe(&tree, shown)).key("flame-line").font(pal::SMALL_NUM).color(p.ink2).single_line().truncate().shrink().min_w(0.0).selectable(true))
        .child(spacer().grow(1.0))
        .child(text(hidden_note(hidden)).key("flame-hidden").font(pal::SMALL).color(p.ink3).single_line());
    vec![crumbs(&tree, root, p), col().w_full().gap(8.0).child(group).child(info).child(legend(p))]
}

/// The path from the session to the shown root, each step a button that shows it, and the shown
/// root's value at the right.
fn crumbs(tree: &Tree, root: usize, p: &Pal) -> El {
    let line = tree.line(root);
    let last = line.len() - 1;
    let mut out = row().key("flame-crumbs").w_full().gap(2.0).center_y();
    for (k, &i) in line.iter().enumerate() {
        if k > 0 {
            out = out.child(text("›").font(pal::BODY).color(p.ink3).single_line());
        }
        let on = k == last;
        let label = text(tree.nodes[i].name.clone())
            .key("crumb-name")
            .font(if on { pal::STRONG } else { pal::BODY })
            .color(if on { p.ink } else { p.ink2 })
            .single_line()
            .truncate();
        let crumb = row()
            .key(format!("crumb-{k}"))
            .h(24.0)
            .px(6.0)
            .radius(PILL)
            .center_y()
            .min_w(0.0)
            .shrink()
            .role("button")
            .label(tree.nodes[i].name.clone())
            .child(label);
        out = out.child(if on { crumb } else { crumb.hover(p.hover).on_press(move |s: &mut Desk| s.flame_crumb(i)) });
    }
    let n = &tree.nodes[root];
    let share = 100.0 * n.value as f64 / tree.total().max(1) as f64;
    out.child(spacer().grow(1.0))
        .child(text(format!("{} · {share:.1}% of the session", tree.amount(n.value))).key("flame-total").font(pal::STRONG_NUM).color(p.ink).single_line())
}

/// A frame's bar: its name inside when it fits, and its value when there is room.
fn bar(tree: &Tree, b: &Bar, cursor: bool, p: &Pal) -> El {
    let n = &tree.nodes[b.node];
    let fill = fill(n.tint, p);
    let i = b.node;
    let path = tree.path(i);
    let mut out = row()
        .key(format!("frame-{i}"))
        .absolute(b.x, b.y)
        .w(b.w)
        .h(ROW)
        .radius(R_ROW.min(b.w / 2.0))
        .fill(fill)
        .hover(fill.mix(Color::WHITE, 0.3))
        .center_y()
        .role("button")
        .label(n.name.clone())
        .on_hover(move |s: &mut Desk| {
            if let Some(x) = s.browser.xray()
                && s.flame.session != x.id
            {
                s.flame = State { session: x.id.clone(), ..State::default() };
            }
            s.flame.hover = Some(path.clone());
        })
        .on_press(move |s: &mut Desk| s.flame_click(i));
    if cursor {
        out = out.outline(2.0, 1.0, p.accent);
    }
    // A narrow bar has no padding, which would widen it.
    if b.w >= LABEL_W {
        out = out.px(6.0).gap(6.0);
        let amount = tree.amount(n.value);
        // A rough measure of the system font at 11.5 pt.
        let fits = |chars: usize| chars as f32 * 6.3 + 12.0 <= b.w;
        out = out.child(text(n.name.clone()).key("frame-name").font(pal::SMALL).color(p.ink).single_line().truncate().shrink().min_w(0.0));
        if fits(n.name.chars().count() + amount.chars().count() + 1) {
            out = out.child(spacer().grow(1.0)).child(text(amount).key("frame-amount").font(pal::SMALL_NUM).color(p.ink2).single_line());
        }
    }
    out
}

/// The colors of the frames, and the keys.
fn legend(p: &Pal) -> El {
    let kinds = [
        ("structure", Tint::Structure),
        ("content", Tint::Content),
        ("read", Tint::Read),
        ("write", Tint::Write),
        ("rewrite", Tint::Miss),
        ("uncached", Tint::Uncached),
        ("output", Tint::Output),
    ];
    let mut out = row().key("flame-legend").w_full().flex_wrap().gaps(6.0, 10.0).center_y();
    for (name, tint) in kinds {
        let swatch = el().w(10.0).h(10.0).radius(R_MARK).fill(fill(tint, p));
        out = out.child(row().gap(5.0).center_y().child(swatch).child(text(name).font(pal::SMALL).color(p.ink3).single_line()));
    }
    let keys = row()
        .key("flame-keys")
        .gap(12.0)
        .center_y()
        .child(text("click zooms").font(pal::SMALL).color(p.ink3).single_line())
        .child(overlay::hint(&["←", "→", "↑", "↓"], "move", p))
        .child(overlay::hint(&["⏎"], "zoom", p))
        .child(overlay::hint(&["esc"], "out", p))
        .child(overlay::hint(&["⇧←", "⇧→"], "tabs", p));
    out.child(spacer().grow(1.0)).child(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::profile::{Frame, Values};

    fn frame(name: &str, tint: Tint, dim: &'static str) -> Frame {
        Frame { name: name.into(), tint, dim }
    }

    /// Stacks under one agent, project and session, by `chain;turn` with dollars.
    fn stacks(rows: &[(&str, &str, f64)]) -> Stacks {
        let mut s = Stacks::new();
        for &(chain, turn, usd) in rows {
            let key = vec![
                frame("claude", Tint::Structure, "agent"),
                frame("~/p", Tint::Structure, "project"),
                frame("ab12", Tint::Structure, "session"),
                frame(chain, Tint::Structure, "chain"),
                frame(turn, Tint::Structure, "turn"),
                frame("read", Tint::Read, "component"),
            ];
            s.entry(key).or_default().add(Values { tokens: usd * 1000.0, usd, avoidable: 0.0 });
        }
        s
    }

    #[test]
    fn the_tree_sums_the_stacks_largest_first_with_the_shared_head_in_the_root() {
        let t = Tree::of(&stacks(&[("main", "turn 1", 1.0), ("main", "turn 2", 2.0), ("subagent: x", "turn 1", 1.0), ("main", "turn 3", 0.0)]), true);
        assert_eq!(t.total(), 4_000_000, "µ$, the stack of $0 left out");
        assert_eq!(t.nodes[0].name, "claude · ~/p · ab12");
        let main = t.find(&[("main".into(), Tint::Structure)]).unwrap();
        assert_eq!(t.nodes[0].children[0], main, "the largest first");
        assert_eq!(t.nodes[main].value, 3_000_000);
        let names: Vec<&str> = t.nodes[main].children.iter().map(|&c| t.nodes[c].name.as_str()).collect();
        assert_eq!(names, ["turn 2", "turn 1"]);
        assert_eq!(t.nodes[0].size, t.nodes.len());
        assert_eq!(t.names(t.nodes[main].children[0]), "claude › ~/p › ab12 › main › turn 2");
        assert_eq!(t.find(&t.path(t.nodes[main].children[1])), Some(t.nodes[main].children[1]));
        let d = describe(&t, t.nodes[main].children[0]);
        assert_eq!(d, "claude › ~/p › ab12 › main › turn 2 · $2.00 · 50.0% of the session · 66.7% of main");
    }

    #[test]
    fn without_prices_the_tree_counts_tokens() {
        let t = Tree::of(&stacks(&[("main", "turn 1", 1.5)]), false);
        assert!(t.in_tokens);
        assert_eq!(t.total(), 1500);
        assert_eq!(t.amount(t.total()), "1.5k tok");
    }

    #[test]
    fn a_head_that_differs_does_not_fold() {
        let mut s = stacks(&[("main", "turn 1", 1.0)]);
        s.insert(vec![frame("other", Tint::Structure, "agent"), frame("main", Tint::Structure, "chain")], Values { tokens: 1.0, usd: 1.0, avoidable: 0.0 });
        let t = Tree::of(&s, true);
        assert!(t.head.is_empty());
        assert_eq!(t.nodes[0].name, "all");
        assert_eq!(t.nodes[0].children.len(), 2);
    }

    #[test]
    fn bars_are_as_wide_as_their_share_and_narrow_ones_are_counted() {
        let mut rows = vec![("main", "turn 1", 3.0), ("main", "turn 2", 0.999)];
        rows.push(("tiny", "turn 1", 0.001));
        let t = Tree::of(&stacks(&rows), true);
        let (bars, hidden) = layout(&t, 0, 998.0);
        let w = |i: usize| bars.iter().find(|b| b.node == i).map(|b| b.w);
        let main = t.find(&[("main".into(), Tint::Structure)]).unwrap();
        assert_eq!(w(0), Some(998.0));
        assert!((w(main).unwrap() - (3.999 / 4.0 * 1000.0 - GAP)).abs() < 0.01, "{:?}", w(main));
        // tiny is 0.25 pt wide: it, its turn and its component are hidden.
        assert_eq!(hidden, 3);
        assert_eq!(hidden_note(hidden), "3 frames under 3 pt hidden");
        let turn1 = t.nodes[main].children[0];
        let (b0, b1) = (bars.iter().find(|b| b.node == turn1).unwrap(), bars.iter().find(|b| b.node == t.nodes[main].children[1]).unwrap());
        assert!((b1.x - (b0.x + b0.w + GAP)).abs() < 0.01, "2 pt between neighbours");
        assert_eq!(b0.y, 2.0 * (ROW + GAP));
        // Zoomed into main, its turns share the full width.
        let (zoomed, hidden) = layout(&t, main, 998.0);
        assert_eq!((zoomed[0].node, zoomed[0].w, zoomed[0].y), (main, 998.0, 0.0));
        assert_eq!(hidden, 0);
    }

    #[test]
    fn the_cursor_moves_through_drawn_frames() {
        let t = Tree::of(&stacks(&[("main", "turn 1", 3.0), ("main", "turn 2", 1.0), ("side", "turn 1", 1.0), ("tiny", "turn 1", 0.0001)]), true);
        let main = t.nodes[0].children[0];
        assert_eq!(step(&t, 0, 600.0, 0, Move::Child), Some(main));
        assert_eq!(step(&t, 0, 600.0, 0, Move::After), None, "the root has no siblings");
        let side = step(&t, 0, 600.0, main, Move::After).unwrap();
        assert_eq!(t.nodes[side].name, "side");
        assert_eq!(step(&t, 0, 600.0, side, Move::After), None, "tiny is not drawn");
        assert_eq!(step(&t, 0, 600.0, side, Move::Before), Some(main));
        assert_eq!(step(&t, 0, 600.0, main, Move::Parent), Some(0));
        assert_eq!(step(&t, main, 600.0, main, Move::Parent), None, "not above the shown root");
    }
}
