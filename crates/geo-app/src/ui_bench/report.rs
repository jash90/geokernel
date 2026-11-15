//! The text table and the JSON file of a benchmark run.

use super::{Frame, Measured, Repaint, Sample};
use std::collections::BTreeSet;
use std::fmt::Write;

fn median(mut v: Vec<u64>) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[v.len() / 2]
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

/// The frames of all rounds of one scene and state, merged.
pub struct Summary<'a> {
    pub scene: &'a str,
    pub state: &'a str,
    pub objects: usize,
    pub on_screen: usize,
    pub frames: Vec<&'a Frame>,
    pub notes: BTreeSet<String>,
}

impl Summary<'_> {
    fn med(&self, f: impl Fn(&Frame) -> u64) -> u64 {
        median(self.frames.iter().map(|x| f(x)).collect())
    }
    fn min(&self, f: impl Fn(&Frame) -> u64) -> u64 {
        self.frames.iter().map(|x| f(x)).min().unwrap_or(0)
    }
    fn max(&self, f: impl Fn(&Frame) -> u64) -> u64 {
        self.frames.iter().map(|x| f(x)).max().unwrap_or(0)
    }
    fn ok(&self) -> usize {
        self.frames.iter().filter(|f| f.ok).count()
    }
    /// `immediate/delayed/none` counts and the shortest delay.
    fn repaint(&self) -> (usize, usize, usize, Option<f64>) {
        let (mut imm, mut later, mut none, mut shortest) = (0, 0, 0, None::<f64>);
        for f in &self.frames {
            match f.repaint {
                Repaint::Immediate => imm += 1,
                Repaint::After(d) => {
                    later += 1;
                    shortest = Some(shortest.map_or(d, |s| s.min(d)));
                }
                Repaint::None => none += 1,
            }
        }
        (imm, later, none, shortest)
    }
}

pub fn summaries(measured: &[Measured]) -> Vec<Summary<'_>> {
    let mut out: Vec<Summary> = Vec::new();
    for m in measured {
        match out
            .iter_mut()
            .find(|s| s.scene == m.scene && s.state == m.state)
        {
            Some(s) => {
                s.frames.extend(m.frames.iter());
                s.notes.extend(m.notes.iter().cloned());
            }
            None => out.push(Summary {
                scene: &m.scene,
                state: m.state,
                objects: m.objects,
                on_screen: m.on_screen,
                frames: m.frames.iter().collect(),
                notes: m.notes.clone(),
            }),
        }
    }
    out
}

/// `(scene, name) -> (count, min ns, median ns, median allocs, median bytes)`.
fn sample_rows(samples: &[Sample]) -> Vec<(&str, &str, usize, u64, u64, u64, u64)> {
    let mut keys: Vec<(&str, &str)> = Vec::new();
    for s in samples {
        if !keys.contains(&(s.scene.as_str(), s.name.as_str())) {
            keys.push((&s.scene, &s.name));
        }
    }
    keys.into_iter()
        .map(|(scene, name)| {
            let mine: Vec<&Sample> = samples
                .iter()
                .filter(|s| s.scene == scene && s.name == name)
                .collect();
            let ns: Vec<u64> = mine.iter().map(|s| s.ns).collect();
            (
                scene,
                name,
                mine.len(),
                ns.iter().copied().min().unwrap_or(0),
                median(ns),
                median(mine.iter().map(|s| s.allocs).collect()),
                median(mine.iter().map(|s| s.bytes).collect()),
            )
        })
        .collect()
}

fn repaint_text(s: &Summary) -> String {
    let (imm, later, none, shortest) = s.repaint();
    let mut t = format!("{imm}i/{later}d/{none}n");
    if let Some(d) = shortest {
        let _ = write!(t, " ({d:.0}ms)");
    }
    t
}

pub fn table(measured: &[Measured], samples: &[Sample]) -> String {
    let mut t = String::new();
    let _ = writeln!(
        t,
        "{:<10} {:<24} {:>6} {:>6} {:>4} {:>8} {:>8} {:>8} {:>8} {:>6} {:>7} {:>7} {:>5} {:>6} {:>4} {:>7} {:>8} {:>4} {:>4} {:>5}  repaint(i/d/n)",
        "scene", "state", "objs", "onscr", "ok", "ui_med", "ui_min", "tes_med", "tes_min",
        "shapes", "verts", "idx", "texts", "glyphs", "new", "allocs", "KB", "rows", "rebl", "recmp"
    );
    for s in summaries(measured) {
        let _ = writeln!(
            t,
            "{:<10} {:<24} {:>6} {:>6} {:>4} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>6} {:>7} {:>7} {:>5} {:>6} {:>4} {:>7} {:>8.1} {:>4} {:>4} {:>5}  {}",
            s.scene,
            s.state,
            s.objects,
            s.on_screen,
            format!("{}/{}", s.ok(), s.frames.len()),
            ms(s.med(|f| f.ui_ns)),
            ms(s.min(|f| f.ui_ns)),
            ms(s.med(|f| f.tess_ns)),
            ms(s.min(|f| f.tess_ns)),
            s.med(|f| f.shapes),
            s.med(|f| f.vertices),
            s.med(|f| f.indices),
            s.med(|f| f.text_shapes),
            s.med(|f| f.glyphs),
            s.med(|f| f.new_galleys),
            s.med(|f| f.ui_allocs + f.tess_allocs),
            s.med(|f| f.ui_bytes + f.tess_bytes) as f64 / 1024.0,
            s.med(|f| f.rows_built),
            s.med(|f| f.rebuilds),
            s.med(|f| f.recomputed),
            repaint_text(&s),
        );
        for n in &s.notes {
            let _ = writeln!(t, "{:<10} {:<24} note: {n}", "", "");
        }
    }
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "{:<10} {:<34} {:>3} {:>10} {:>10} {:>9} {:>10}",
        "scene", "action / helper", "n", "min_ms", "med_ms", "allocs", "KB"
    );
    for (scene, name, n, min, med, allocs, bytes) in sample_rows(samples) {
        let _ = writeln!(
            t,
            "{scene:<10} {name:<34} {n:>3} {:>10.3} {:>10.3} {allocs:>9} {:>10.1}",
            ms(min),
            ms(med),
            bytes as f64 / 1024.0
        );
    }
    t
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A hand-written JSON document (the crate has no serde).
pub fn json(
    tree: &str,
    frames: usize,
    rounds: usize,
    measured: &[Measured],
    samples: &[Sample],
) -> String {
    let mut j = String::new();
    let _ = write!(
        j,
        "{{\n  \"tree\": \"{}\",\n  \"frames_per_round\": {frames},\n  \"rounds\": {rounds},\n  \"states\": [\n",
        esc(tree)
    );
    let all = summaries(measured);
    for (i, s) in all.iter().enumerate() {
        let (imm, later, none, shortest) = s.repaint();
        let counters = [
            (
                "shapes",
                s.med(|f| f.shapes),
                s.min(|f| f.shapes),
                s.max(|f| f.shapes),
            ),
            (
                "vertices",
                s.med(|f| f.vertices),
                s.min(|f| f.vertices),
                s.max(|f| f.vertices),
            ),
            (
                "indices",
                s.med(|f| f.indices),
                s.min(|f| f.indices),
                s.max(|f| f.indices),
            ),
            (
                "meshes",
                s.med(|f| f.meshes),
                s.min(|f| f.meshes),
                s.max(|f| f.meshes),
            ),
            (
                "text_shapes",
                s.med(|f| f.text_shapes),
                s.min(|f| f.text_shapes),
                s.max(|f| f.text_shapes),
            ),
            (
                "glyphs",
                s.med(|f| f.glyphs),
                s.min(|f| f.glyphs),
                s.max(|f| f.glyphs),
            ),
            (
                "new_galleys",
                s.med(|f| f.new_galleys),
                s.min(|f| f.new_galleys),
                s.max(|f| f.new_galleys),
            ),
            (
                "ui_allocs",
                s.med(|f| f.ui_allocs),
                s.min(|f| f.ui_allocs),
                s.max(|f| f.ui_allocs),
            ),
            (
                "ui_bytes",
                s.med(|f| f.ui_bytes),
                s.min(|f| f.ui_bytes),
                s.max(|f| f.ui_bytes),
            ),
            (
                "tess_allocs",
                s.med(|f| f.tess_allocs),
                s.min(|f| f.tess_allocs),
                s.max(|f| f.tess_allocs),
            ),
            (
                "tess_bytes",
                s.med(|f| f.tess_bytes),
                s.min(|f| f.tess_bytes),
                s.max(|f| f.tess_bytes),
            ),
            (
                "passes",
                s.med(|f| f.passes),
                s.min(|f| f.passes),
                s.max(|f| f.passes),
            ),
            (
                "texture_updates",
                s.med(|f| f.texture_updates),
                s.min(|f| f.texture_updates),
                s.max(|f| f.texture_updates),
            ),
            (
                "rows_built",
                s.med(|f| f.rows_built),
                s.min(|f| f.rows_built),
                s.max(|f| f.rows_built),
            ),
            (
                "rebuilds",
                s.med(|f| f.rebuilds),
                s.min(|f| f.rebuilds),
                s.max(|f| f.rebuilds),
            ),
            (
                "recomputed",
                s.med(|f| f.recomputed),
                s.min(|f| f.recomputed),
                s.max(|f| f.recomputed),
            ),
        ];
        let _ = write!(
            j,
            "    {{\"scene\": \"{}\", \"state\": \"{}\", \"objects\": {}, \"on_screen\": {}, \"frames\": {}, \"ok\": {},\n",
            esc(s.scene),
            s.state,
            s.objects,
            s.on_screen,
            s.frames.len(),
            s.ok()
        );
        let _ = write!(
            j,
            "     \"ui_ms\": {{\"min\": {:.4}, \"median\": {:.4}}}, \"tess_ms\": {{\"min\": {:.4}, \"median\": {:.4}}},\n",
            ms(s.min(|f| f.ui_ns)),
            ms(s.med(|f| f.ui_ns)),
            ms(s.min(|f| f.tess_ns)),
            ms(s.med(|f| f.tess_ns))
        );
        let _ = write!(
            j,
            "     \"repaint\": {{\"immediate\": {imm}, \"delayed\": {later}, \"none\": {none}, \"shortest_delay_ms\": {}}},\n     \"counters\": {{",
            shortest.map_or("null".to_owned(), |d| format!("{d:.1}"))
        );
        for (k, (name, med, min, max)) in counters.iter().enumerate() {
            let sep = if k == 0 { "" } else { ", " };
            let _ = write!(
                j,
                "{sep}\"{name}\": {{\"median\": {med}, \"min\": {min}, \"max\": {max}}}"
            );
        }
        let notes: Vec<String> = s.notes.iter().map(|n| format!("\"{}\"", esc(n))).collect();
        let sep = if i + 1 == all.len() { "" } else { "," };
        let _ = writeln!(j, "}},\n     \"notes\": [{}]}}{sep}", notes.join(", "));
    }
    let _ = writeln!(j, "  ],\n  \"actions\": [");
    let rows = sample_rows(samples);
    for (i, (scene, name, n, min, med, allocs, bytes)) in rows.iter().enumerate() {
        let sep = if i + 1 == rows.len() { "" } else { "," };
        let _ = writeln!(
            j,
            "    {{\"scene\": \"{}\", \"name\": \"{}\", \"n\": {n}, \"min_ms\": {:.4}, \"median_ms\": {:.4}, \"allocs\": {allocs}, \"bytes\": {bytes}}}{sep}",
            esc(scene),
            esc(name),
            ms(*min),
            ms(*med)
        );
    }
    let _ = writeln!(j, "  ]\n}}");
    j
}
