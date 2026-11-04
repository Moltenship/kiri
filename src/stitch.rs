use std::cmp::Reverse;
use std::ops::Range;

const BLOCKS: usize = 32;
const MEAN_TOLERANCE: u8 = 3;
const PIXEL_TOLERANCE: u8 = 32;
/// Changed values a row signature may have and still match, which absorbs a caret, cursor or scrollbar.
const MAX_CHANGED: usize = 2;
/// A mismatched row outweighs this many matched ones, so blank rows can't buy a wrong shift.
const MISS_PENALTY: usize = 16;
const VERIFIED_CANDIDATES: usize = 16;

/// Means catch thin features such as text; samples tell apart rows of texture whose means are alike.
#[derive(Clone, Copy)]
struct Signature {
    means: [u8; BLOCKS],
    samples: [u8; BLOCKS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `rows` is less than the movement once `max_height` is reached, and 0 after that.
    Appended {
        rows: u32,
    },
    NoMovement,
    Failed,
}

pub struct Stitcher {
    width: usize,
    height: usize,
    max_height: usize,
    prev: Vec<u8>,
    prev_signatures: Vec<Signature>,
    next_signatures: Vec<Signature>,
    out: Vec<u8>,
}

impl Stitcher {
    /// First frame. Frames are tightly packed BGRA8, all the same width and height.
    pub fn new(first: &[u8], width: u32, height: u32, max_height: u32) -> Self {
        let (width, height) = (width as usize, height as usize);
        assert!(width > 0 && height > 0);
        assert_eq!(
            first.len(),
            width * height * 4,
            "frame is not width * height BGRA8"
        );
        let mut prev_signatures = Vec::with_capacity(height);
        signatures(first, width, &mut prev_signatures);
        Self {
            width,
            height,
            max_height: (max_height as usize).max(height),
            prev: first.to_vec(),
            prev_signatures,
            next_signatures: Vec::with_capacity(height),
            out: first.to_vec(),
        }
    }

    /// Feed the next frame. Finds how far content moved relative to the previous frame and appends the new rows.
    pub fn push(&mut self, frame: &[u8]) -> Step {
        assert_eq!(frame.len(), self.prev.len(), "frame size changed");
        signatures(frame, self.width, &mut self.next_signatures);
        let h = self.height;
        let unchanged = |y: &usize| similar(&self.prev_signatures[*y], &self.next_signatures[*y]);
        let top = (0..h).take_while(unchanged).count();
        if top == h {
            return Step::NoMovement;
        }
        let bottom = (top..h).rev().take_while(unchanged).count();
        let Some(shift) = self.find_shift(frame, top, bottom) else {
            return Step::Failed;
        };

        // `out` ends with the previous frame's bottom rows; they give way to the new rows and this frame's bottom.
        let rows = shift.min(self.max_height - self.height() as usize);
        if rows > 0 {
            let stride = self.width * 4;
            let start = h - bottom - shift;
            self.out.truncate(self.out.len() - bottom * stride);
            self.out
                .extend_from_slice(&frame[start * stride..(start + rows) * stride]);
            self.out.extend_from_slice(&frame[(h - bottom) * stride..]);
        }
        self.prev.copy_from_slice(frame);
        std::mem::swap(&mut self.prev_signatures, &mut self.next_signatures);
        Step::Appended { rows: rows as u32 }
    }

    pub fn height(&self) -> u32 {
        (self.out.len() / (self.width * 4)) as u32
    }

    /// The stitched image so far, tightly packed BGRA8, width * height() * 4 bytes.
    pub fn finish(self) -> (u32, u32, Vec<u8>) {
        (self.width as u32, self.height(), self.out)
    }

    /// Rows `top..h - bottom` changed; the rest are identical in both frames,
    /// either sticky bars or content that looks the same after the shift.
    fn find_shift(&self, frame: &[u8], top: usize, bottom: usize) -> Option<usize> {
        let (prev, next) = (&self.prev_signatures, &self.next_signatures);
        let h = self.height;
        let end = h - bottom;
        let matches = |y: usize, shift: usize| similar(&next[y], &prev[y + shift]);
        let min_overlap = (h / 16).max(8);

        let mut candidates = Vec::new();
        'shift: for shift in 1..end - top {
            let core = top..end - shift;
            let budget = core.len() / MISS_PENALTY;
            let mut misses = 0;
            for y in core {
                if !matches(y, shift) {
                    misses += 1;
                    if misses > budget {
                        continue 'shift;
                    }
                }
            }
            // Unchanged rows around the changed band are evidence while they also match shifted;
            // on a sparse page the band alone is too short to trust.
            let up = (0..top).rev().take_while(|&y| matches(y, shift)).count();
            let down = (end - shift..h - shift)
                .take_while(|&y| matches(y, shift))
                .count();
            let overlap = top - up..end - shift + down;
            let score = overlap.len().saturating_sub(MISS_PENALTY * misses);
            if score >= min_overlap {
                candidates.push((score, shift, overlap));
            }
        }
        candidates.sort_by_key(|(score, shift, _)| (Reverse(*score), *shift));
        candidates
            .into_iter()
            .take(VERIFIED_CANDIDATES)
            .find(|(_, shift, overlap)| {
                // A shift is only pinned down by an edge between two rows that both match.
                let structured = (overlap.start..overlap.end - 1).any(|y| {
                    !similar(&next[y], &next[y + 1]) && matches(y, *shift) && matches(y + 1, *shift)
                });
                structured && self.pixels_match(frame, *shift, overlap.clone())
            })
            .map(|(_, shift, _)| shift)
    }

    fn pixels_match(&self, frame: &[u8], shift: usize, rows: Range<usize>) -> bool {
        let stride = self.width * 4;
        let budget = rows.len() / MISS_PENALTY;
        let mut misses = 0;
        for y in rows {
            let next = &frame[y * stride..][..stride];
            let prev = &self.prev[(y + shift) * stride..][..stride];
            let changed: u32 = next
                .iter()
                .zip(prev)
                .map(|(a, b)| u32::from(a.abs_diff(*b) > PIXEL_TOLERANCE))
                .sum();
            if changed as usize > stride / 64 {
                misses += 1;
                if misses > budget {
                    return false;
                }
            }
        }
        true
    }
}

fn signatures(frame: &[u8], width: usize, out: &mut Vec<Signature>) {
    let bounds: [usize; BLOCKS + 1] = std::array::from_fn(|k| k * width / BLOCKS);
    out.clear();
    out.extend(frame.chunks_exact(width * 4).map(|row| Signature {
        means: std::array::from_fn(|k| {
            let block = &row[bounds[k] * 4..bounds[k + 1] * 4];
            let sum: u32 = block.iter().map(|&b| u32::from(b)).sum();
            (sum / block.len().max(1) as u32) as u8
        }),
        samples: std::array::from_fn(|k| {
            let x = ((bounds[k] + bounds[k + 1]) / 2).min(width - 1);
            let (b, g, r) = (row[x * 4], row[x * 4 + 1], row[x * 4 + 2]);
            ((u16::from(b) + 2 * u16::from(g) + u16::from(r)) / 4) as u8
        }),
    }));
}

fn similar(a: &Signature, b: &Signature) -> bool {
    let means = a
        .means
        .iter()
        .zip(&b.means)
        .filter(|(x, y)| x.abs_diff(**y) > MEAN_TOLERANCE);
    let samples = a
        .samples
        .iter()
        .zip(&b.samples)
        .filter(|(x, y)| x.abs_diff(**y) > PIXEL_TOLERANCE);
    means.count() + samples.count() <= MAX_CHANGED
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const W: usize = 320;
    const H: usize = 240;
    const HEADER: usize = 24;
    const FOOTER: usize = 16;

    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }
    }

    fn put(image: &mut [u8], width: usize, x: usize, y: usize, px: [u8; 4]) {
        let i = (y * width + x) * 4;
        if let Some(dst) = image.get_mut(i..i + 4) {
            dst.copy_from_slice(&px);
        }
    }

    fn blank(width: usize, height: usize) -> Vec<u8> {
        [250, 248, 245, 255].repeat(width * height)
    }

    /// Text lines of noisy "glyphs", paragraph gaps, images and rules.
    fn dense_page(width: usize, height: usize, seed: u64) -> Vec<u8> {
        let mut rng = Rng(seed);
        let mut page = blank(width, height);
        let mut y = 0;
        while y < height {
            y += match rng.below(12) {
                0 => {
                    let rows = 40 + rng.below(120);
                    let (x0, x1) = (rng.below(width / 4), width - rng.below(width / 4));
                    for yy in y..y + rows {
                        for x in x0..x1 {
                            let n = rng.below(48) as u8;
                            put(
                                &mut page,
                                width,
                                x,
                                yy,
                                [
                                    (x as u8).wrapping_mul(3) ^ n,
                                    (yy as u8).wrapping_mul(5),
                                    100 + n,
                                    255,
                                ],
                            );
                        }
                    }
                    rows + 8
                }
                1 | 2 => 8 + rng.below(32),
                3 => {
                    for x in 0..width {
                        put(&mut page, width, x, y + 4, [180, 180, 180, 255]);
                    }
                    9
                }
                _ => {
                    let line = 16 + rng.below(12);
                    let right = width - rng.below(width / 2);
                    let mut x = 8 + rng.below(24);
                    while x + 8 < right {
                        let len = (4 + rng.below(70)).min(right - x);
                        for yy in y + 3..y + line - 4 {
                            for xx in x..x + len {
                                if rng.below(3) == 0 {
                                    let ink = 20 + rng.below(60) as u8;
                                    put(&mut page, width, xx, yy, [ink, ink, ink, 255]);
                                }
                            }
                        }
                        x += len + 5 + rng.below(10);
                    }
                    line
                }
            };
        }
        page
    }

    /// Plain background with small noisy features every 40-100 rows.
    fn sparse_page(width: usize, height: usize, seed: u64) -> Vec<u8> {
        let mut rng = Rng(seed);
        let mut page = blank(width, height);
        let mut y = 20;
        while y < height {
            let rows = 1 + rng.below(20);
            let x0 = rng.below(width / 2);
            let x1 = x0 + width / 8 + rng.below(width / 2 - width / 8);
            for yy in y..y + rows {
                for x in x0..x1 {
                    let n = rng.below(200) as u8;
                    put(&mut page, width, x, yy, [n, 255 - n, 90, 255]);
                }
            }
            y += rows + 40 + rng.below(60);
        }
        page
    }

    fn bar(frame: &mut [u8], width: usize, rows: Range<usize>, tone: u8) {
        for y in rows {
            for x in 0..width {
                put(
                    frame,
                    width,
                    x,
                    y,
                    [
                        tone.wrapping_add((x / 7) as u8),
                        60 + (y % 5) as u8 * 9,
                        ((x ^ y) / 4) as u8,
                        255,
                    ],
                );
            }
        }
    }

    fn frame(page: &[u8], width: usize, height: usize, offset: usize) -> Vec<u8> {
        let stride = width * 4;
        let mut frame = page[offset * stride..(offset + height) * stride].to_vec();
        bar(&mut frame, width, 0..HEADER, 40);
        bar(&mut frame, width, height - FOOTER..height, 140);
        frame
    }

    /// Header once, the page content seen from `first` down to the bottom of `last`, footer once.
    fn expected(page: &[u8], width: usize, height: usize, first: usize, last: usize) -> Vec<u8> {
        let stride = width * 4;
        let bars = frame(page, width, height, first);
        [
            &bars[..HEADER * stride],
            &page[(first + HEADER) * stride..(last + height - FOOTER) * stride],
            &bars[(height - FOOTER) * stride..],
        ]
        .concat()
    }

    fn flip(image: &[u8], width: usize) -> Vec<u8> {
        image
            .chunks_exact(width * 4)
            .rev()
            .flatten()
            .copied()
            .collect()
    }

    fn offsets(start: usize, steps: &[usize]) -> Vec<usize> {
        steps
            .iter()
            .scan(start, |at, step| {
                *at += step;
                Some(*at)
            })
            .collect()
    }

    fn assert_image(actual: &[u8], expected: &[u8], width: usize) {
        let stride = width * 4;
        assert_eq!(actual.len() / stride, expected.len() / stride, "row count");
        let bad = actual
            .chunks(stride)
            .zip(expected.chunks(stride))
            .position(|(a, b)| a != b);
        assert_eq!(bad, None, "first mismatching row");
    }

    fn stitch(
        page: &[u8],
        width: usize,
        height: usize,
        offsets: &[usize],
        max_height: u32,
    ) -> (Vec<Step>, Vec<u8>) {
        let mut stitcher = Stitcher::new(
            &frame(page, width, height, offsets[0]),
            width as u32,
            height as u32,
            max_height,
        );
        let steps = offsets[1..]
            .iter()
            .map(|&o| stitcher.push(&frame(page, width, height, o)))
            .collect();
        (steps, stitcher.finish().2)
    }

    fn appended(steps: &[usize]) -> Vec<Step> {
        steps
            .iter()
            .map(|&rows| Step::Appended { rows: rows as u32 })
            .collect()
    }

    #[test]
    fn varying_steps_with_sticky_bars() {
        let page = dense_page(W, 4000, 1);
        let moves = [1, 2, 3, 7, 16, 33, 64, 90, 128, 160, 1, 5, 120, 170, 2];
        let at = offsets(50, &[0].iter().chain(&moves).copied().collect::<Vec<_>>());
        let (steps, out) = stitch(&page, W, H, &at, u32::MAX);
        assert_eq!(steps, appended(&moves));
        assert_image(&out, &expected(&page, W, H, at[0], *at.last().unwrap()), W);
    }

    #[test]
    fn no_movement_and_jump() {
        let page = dense_page(W, 4000, 2);
        let (steps, out) = stitch(&page, W, H, &[100, 140, 140, 1340, 190], u32::MAX);
        assert_eq!(
            steps,
            [
                Step::Appended { rows: 40 },
                Step::NoMovement,
                Step::Failed,
                Step::Appended { rows: 50 }
            ]
        );
        assert_image(&out, &expected(&page, W, H, 100, 190), W);
    }

    #[test]
    fn sparse_features() {
        let page = sparse_page(W, 4000, 3);
        let moves = [4, 30, 80, 1, 9, 60, 100, 2, 45];
        let at = offsets(0, &[0].iter().chain(&moves).copied().collect::<Vec<_>>());
        let (steps, out) = stitch(&page, W, H, &at, u32::MAX);
        assert_eq!(steps, appended(&moves));
        assert_image(&out, &expected(&page, W, H, 0, *at.last().unwrap()), W);
    }

    #[test]
    fn scroll_up_by_flipping() {
        let page = dense_page(W, 4000, 4);
        let at = [2000, 1990, 1900, 1870, 1700, 1699];
        let mut stitcher = Stitcher::new(
            &flip(&frame(&page, W, H, at[0]), W),
            W as u32,
            H as u32,
            u32::MAX,
        );
        for &o in &at[1..] {
            assert!(matches!(
                stitcher.push(&flip(&frame(&page, W, H, o), W)),
                Step::Appended { .. }
            ));
        }
        let out = flip(&stitcher.finish().2, W);
        assert_image(&out, &expected(&page, W, H, 1699, 2000), W);
    }

    #[test]
    fn stops_at_max_height() {
        let page = dense_page(W, 4000, 5);
        let (steps, out) = stitch(&page, W, H, &[0, 100, 250, 400], 400);
        assert_eq!(steps, appended(&[100, 60, 0]));
        let stride = W * 4;
        let full = expected(&page, W, H, 0, 400);
        let tail = &full[full.len() - FOOTER * stride..];
        assert_image(&out, &[&full[..(400 - FOOTER) * stride], tail].concat(), W);
    }

    #[test]
    fn tolerates_noise_and_caret() {
        let page = dense_page(W, 4000, 6);
        let moves = [3, 40, 90, 12, 150, 1];
        let at = offsets(300, &[0].iter().chain(&moves).copied().collect::<Vec<_>>());
        let mut rng = Rng(7);
        let mut noisy = |offset: usize, caret: bool| {
            let mut f = frame(&page, W, H, offset);
            for px in f.as_chunks_mut::<4>().0 {
                for c in &mut px[..3] {
                    *c = (*c as i32 + rng.below(9) as i32 - 4).clamp(0, 255) as u8;
                }
            }
            for y in 110..128 {
                for x in 100..102 {
                    put(
                        &mut f,
                        W,
                        x,
                        y,
                        if caret {
                            [0, 0, 0, 255]
                        } else {
                            [250, 248, 245, 255]
                        },
                    );
                }
            }
            f
        };
        let mut stitcher = Stitcher::new(&noisy(at[0], false), W as u32, H as u32, u32::MAX);
        let steps: Vec<_> = at[1..]
            .iter()
            .enumerate()
            .map(|(i, &o)| stitcher.push(&noisy(o, i % 2 == 0)))
            .collect();
        assert_eq!(steps, appended(&moves));
        let out = stitcher.finish().2;
        let clean = expected(&page, W, H, at[0], *at.last().unwrap());
        assert_eq!(out.len(), clean.len());
        let off = out
            .iter()
            .zip(&clean)
            .filter(|(a, b)| a.abs_diff(**b) > 8)
            .count();
        assert!(off < out.len() / 1000, "{off} bytes off");
    }

    #[test]
    fn random_steps_never_misalign() {
        for seed in 1..40 {
            for sparse in [false, true] {
                let page = if sparse {
                    sparse_page(W, 6000, seed)
                } else {
                    dense_page(W, 6000, seed)
                };
                let mut rng = Rng(seed * 77 + 1);
                let moves: Vec<_> = (0..25)
                    .map(|_| 1 + rng.below(if sparse { 90 } else { 175 }))
                    .collect();
                let at = offsets(0, &[0].iter().chain(&moves).copied().collect::<Vec<_>>());
                let (steps, out) = stitch(&page, W, H, &at, u32::MAX);
                // A step over only blank overlap is honestly ambiguous; it must fail rather than guess.
                let Some(failed) = steps.iter().position(|s| *s == Step::Failed) else {
                    assert_eq!(steps, appended(&moves), "seed {seed}");
                    assert_image(&out, &expected(&page, W, H, 0, *at.last().unwrap()), W);
                    continue;
                };
                assert!(
                    !sparse && moves[failed] > 100,
                    "seed {seed} failed on a {} px move",
                    moves[failed]
                );
                assert_eq!(steps[..failed], appended(&moves[..failed]), "seed {seed}");
            }
        }
    }

    fn time_pushes(page: &[u8], moves: &[usize]) -> Vec<Step> {
        let (w, h) = (2880, 1800);
        let at = offsets(0, &[0].iter().chain(moves).copied().collect::<Vec<_>>());
        let frames: Vec<_> = at.iter().map(|&o| frame(page, w, h, o)).collect();
        let mut stitcher = Stitcher::new(&frames[0], w as u32, h as u32, u32::MAX);
        let mut times = Vec::new();
        let steps = frames[1..]
            .iter()
            .map(|f| {
                let start = Instant::now();
                let step = stitcher.push(f);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                step
            })
            .collect();
        let worst = times.iter().copied().fold(0.0, f64::max);
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        eprintln!("2880x1800 push: mean {mean:.2} ms, worst {worst:.2} ms");
        if steps == appended(moves) {
            assert_image(
                &stitcher.finish().2,
                &expected(page, w, h, 0, *at.last().unwrap()),
                w,
            );
        }
        steps
    }

    #[test]
    fn retina_timing() {
        let w = 2880;
        let moves = [40, 400, 7, 900, 120, 1200, 64, 300, 1, 1500];
        assert_eq!(
            time_pushes(&dense_page(w, 9000, 8), &moves),
            appended(&moves)
        );

        let mut nearly_blank = blank(w, 7200);
        for y in [500, 1900, 2600, 4000, 5600] {
            bar(&mut nearly_blank, w, y..y + 30, 90);
        }
        let moves = [40, 80, 7, 60, 1, 90, 30, 400, 900, 200];
        assert_eq!(time_pushes(&nearly_blank, &moves), appended(&moves));
    }
}
