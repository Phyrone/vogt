//! Lays a history out as a line map: which lane each commit sits in and how
//! the lines between commits bend.
//!
//! This is the layout the real history is drawn with. `src/lib/graph/layout.ts`
//! is the same algorithm for the synthetic history shown outside the Tauri
//! shell; a change to one belongs in the other.

/// One commit as the layout sees it. Rows are newest first.
pub struct Station {
    /// Index of the line the commit sits on.
    pub line: usize,
    /// The commit's parents in git's order, as rows. `None` is a parent that
    /// is not among the rows.
    pub parents: Vec<Option<usize>>,
}

#[derive(Debug, PartialEq)]
pub struct Edge {
    /// Row of the child.
    pub from: usize,
    /// Row of the parent.
    pub to: usize,
    /// The line whose colour the edge takes: the one that leaves or joins the other.
    pub line: usize,
    /// Corner points as `[lane, row]`.
    pub points: Vec<[usize; 2]>,
}

#[derive(Debug, Default)]
pub struct Layout {
    pub lanes: usize,
    /// The lane of each row.
    pub lane: Vec<usize>,
    /// Ordered by the row they start in.
    pub edges: Vec<Edge>,
}

/// Each line keeps one lane for as long as it runs: `trunk` takes lane 0,
/// every other line the innermost lane that is free between where it leaves
/// its parent line and where it rejoins. `lines` is the number of lines.
pub fn layout(stations: &[Station], lines: usize, trunk: usize) -> Layout {
    // rows each line needs its lane for
    let mut spans: Vec<Option<(usize, usize)>> = vec![None; lines];
    let mut widen = |line: usize, from: usize, to: usize| {
        let span = spans[line].get_or_insert((from, to));
        *span = (span.0.min(from), span.1.max(to));
    };
    for (row, station) in stations.iter().enumerate() {
        widen(station.line, row, row);
        for (nth, parent) in station.parents.iter().enumerate() {
            let Some(parent) = *parent else { continue };
            let parent_line = stations[parent].line;
            if parent_line == station.line {
                continue;
            }
            // the outer line is the one that turns, so it holds its lane up to the bend
            if parent_line == trunk || nth == 0 {
                widen(station.line, row, parent - 1);
            } else {
                widen(parent_line, row + 1, parent);
            }
        }
    }

    let mut order: Vec<(usize, (usize, usize))> = spans
        .iter()
        .enumerate()
        .filter_map(|(line, span)| Some((line, (*span)?)))
        .filter(|(line, _)| *line != trunk)
        .collect();
    order.sort_by_key(|(_, span)| span.0);
    let mut lane_of_line = vec![0; lines];
    // the row each lane is taken up to; lines arrive in the order they start,
    // so a lane is free for one exactly when everything in it has ended
    let mut taken_until: Vec<Option<usize>> = vec![None];
    for (line, (from, to)) in order {
        let mut lane = 1;
        while taken_until
            .get(lane)
            .copied()
            .flatten()
            .is_some_and(|end| end >= from)
        {
            lane += 1;
        }
        if lane == taken_until.len() {
            taken_until.push(None);
        }
        taken_until[lane] = Some(taken_until[lane].map_or(to, |end| end.max(to)));
        lane_of_line[line] = lane;
    }

    let lane: Vec<usize> = stations.iter().map(|s| lane_of_line[s.line]).collect();
    let mut edges = Vec::new();
    for (row, station) in stations.iter().enumerate() {
        let a = lane[row];
        for parent in station.parents.iter().flatten().copied() {
            let b = lane[parent];
            let points = if a == b {
                vec![[a, row], [b, parent]]
            } else if a > b {
                vec![
                    [a, row],
                    [a, row.max(parent.saturating_sub(a - b))],
                    [b, parent],
                ]
            } else {
                vec![[a, row], [b, parent.min(row + (b - a))], [b, parent]]
            };
            edges.push(Edge {
                from: row,
                to: parent,
                line: if a >= b {
                    station.line
                } else {
                    stations[parent].line
                },
                points,
            });
        }
    }

    Layout {
        lanes: taken_until.len(),
        lane,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: usize = 0;
    const FEAT: usize = 1;

    fn station(line: usize, parents: &[usize]) -> Station {
        Station {
            line,
            parents: parents.iter().map(|row| Some(*row)).collect(),
        }
    }

    #[test]
    fn the_trunk_keeps_lane_0_and_a_branch_takes_the_next() {
        let rows = [station(FEAT, &[1]), station(FEAT, &[2]), station(MAIN, &[])];
        let layout = layout(&rows, 2, MAIN);
        assert_eq!(layout.lanes, 2);
        assert_eq!(layout.lane, [1, 1, 0]);
    }

    #[test]
    fn a_branch_bends_into_its_parent_line_at_45_degrees() {
        let rows = [
            station(FEAT, &[3]),
            station(MAIN, &[2]),
            station(MAIN, &[3]),
            station(MAIN, &[]),
        ];
        let layout = layout(&rows, 2, MAIN);
        let edge = &layout.edges[0];
        assert_eq!(edge.line, FEAT);
        // runs down its own lane, then crosses one lane over one row
        assert_eq!(edge.points, [[1, 0], [1, 2], [0, 3]]);
    }

    #[test]
    fn a_merge_edge_takes_the_colour_of_the_merged_line() {
        let rows = [
            station(MAIN, &[2, 1]),
            station(FEAT, &[2]),
            station(MAIN, &[]),
        ];
        let layout = layout(&rows, 2, MAIN);
        let merged = layout.edges.iter().find(|e| e.to == 1).unwrap();
        assert_eq!(merged.line, FEAT);
    }

    #[test]
    fn a_lane_is_reused_once_the_line_that_held_it_has_rejoined() {
        // two branches one after the other, and a third that overlaps the second
        let rows = [
            station(1, &[2]),
            station(3, &[4]),
            station(MAIN, &[3]),
            station(MAIN, &[5, 4]),
            station(2, &[5]),
            station(MAIN, &[]),
        ];
        let layout = layout(&rows, 4, MAIN);
        assert_eq!(layout.lane, [1, 2, 0, 0, 1, 0]);
        assert_eq!(layout.lanes, 3);
    }

    #[test]
    fn a_crossing_edge_stops_at_the_parent_row_when_the_lane_distance_is_larger() {
        // A line crossing several lanes can travel further down than its
        // parent's row; the corner must sit at the parent, not underflow.
        let rows = [
            station(1, &[7, 5]),
            station(3, &[7]),
            station(4, &[3]),
            station(0, &[4]),
            station(0, &[6]),
            station(2, &[7]),
            station(0, &[7]),
            station(0, &[]),
        ];
        let layout = layout(&rows, 5, MAIN);
        let edge = layout
            .edges
            .iter()
            .find(|e| e.from == 2 && e.to == 3)
            .unwrap();
        assert_eq!(edge.points, [[4, 2], [4, 2], [0, 3]]);
    }

    #[derive(serde::Deserialize)]
    struct Golden {
        trunk: String,
        /// The line names; a line's index here is its index for the layout.
        lines: Vec<String>,
        commits: Vec<GoldenCommit>,
        lanes: usize,
        lane: Vec<usize>,
        edges: Vec<GoldenEdge>,
    }

    #[derive(serde::Deserialize)]
    struct GoldenCommit {
        hash: String,
        line: String,
        parents: Vec<String>,
    }

    #[derive(serde::Deserialize)]
    struct GoldenEdge {
        from: usize,
        to: usize,
        line: String,
        points: Vec<[usize; 2]>,
    }

    /// The same fixture is laid out by `src/lib/graph/layout.ts`, so the two
    /// layouts cannot drift apart.
    #[test]
    fn the_golden_history_is_laid_out_as_the_webview_does() {
        let golden: Golden =
            serde_json::from_str(include_str!("../../src/lib/graph/layout.golden.json")).unwrap();
        let index = |line: &str| golden.lines.iter().position(|l| l == line).unwrap();
        let rows: Vec<Station> = golden
            .commits
            .iter()
            .map(|commit| Station {
                line: index(&commit.line),
                parents: commit
                    .parents
                    .iter()
                    .map(|parent| golden.commits.iter().position(|c| &c.hash == parent))
                    .collect(),
            })
            .collect();

        let mut layout = layout(&rows, golden.lines.len(), index(&golden.trunk));
        assert_eq!(layout.lanes, golden.lanes);
        assert_eq!(layout.lane, golden.lane);
        layout.edges.sort_by_key(|edge| (edge.from, edge.to));
        let expected: Vec<Edge> = golden
            .edges
            .into_iter()
            .map(|edge| Edge {
                from: edge.from,
                to: edge.to,
                line: index(&edge.line),
                points: edge.points,
            })
            .collect();
        assert_eq!(layout.edges, expected);
    }

    #[test]
    fn a_parent_outside_the_rows_draws_no_edge() {
        let rows = [Station {
            line: MAIN,
            parents: vec![None],
        }];
        let layout = layout(&rows, 1, MAIN);
        assert!(layout.edges.is_empty());
        assert_eq!(layout.lanes, 1);
    }
}
