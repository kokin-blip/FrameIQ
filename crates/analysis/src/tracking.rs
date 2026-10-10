//! Conservative shot-local association. Scores are heuristics, never identity probabilities.
use frameiq_contracts::RegionRectV1;
#[derive(Clone, Debug)]
pub struct Candidate {
    pub rectangle: RegionRectV1,
    pub appearance: [f64; 3],
}
#[derive(Debug, PartialEq)]
pub enum Association {
    Matched(usize),
    Stopped(&'static str),
}
pub fn iou(a: &RegionRectV1, b: &RegionRectV1) -> f64 {
    let area = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let height = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    let intersection = area.max(0.) * height.max(0.);
    intersection / (a.width * a.height + b.width * b.height - intersection).max(1e-9)
}
/// Association stops permanently at cuts/gaps/ambiguity. A user correction starts a new track.
pub fn associate(
    previous: &Candidate,
    candidates: &[Candidate],
    scene_similarity: f64,
    gap: f64,
) -> Association {
    if !gap.is_finite() || gap < 0. || gap > 1.5 {
        return Association::Stopped("sampling_gap");
    }
    if !scene_similarity.is_finite() || scene_similarity < 0.25 {
        return Association::Stopped("scene_cut");
    }
    let mut scores: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let overlap = iou(&previous.rectangle, &c.rectangle);
            let distance = ((previous.rectangle.x + previous.rectangle.width / 2.
                - c.rectangle.x
                - c.rectangle.width / 2.)
                .powi(2)
                + (previous.rectangle.y + previous.rectangle.height / 2.
                    - c.rectangle.y
                    - c.rectangle.height / 2.)
                    .powi(2))
            .sqrt();
            let appearance = previous
                .appearance
                .iter()
                .zip(c.appearance)
                .map(|(a, b)| (a - b).abs())
                .sum::<f64>()
                / 3.;
            if overlap < 0.25 || distance > 0.2 || appearance > 0.25 {
                return None;
            }
            Some((i, overlap * 0.7 + (1. - appearance) * 0.3))
        })
        .collect();
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    match scores.as_slice() {
        [] => Association::Stopped("subject_lost"),
        [a, b, ..] if a.1 - b.1 < 0.12 => Association::Stopped("ambiguous_subjects"),
        [a, ..] => Association::Matched(a.0),
    }
}
pub fn appearance(image: &image::DynamicImage, rect: &RegionRectV1) -> [f64; 3] {
    let rgb = image.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut sum = [0.; 3];
    let mut count: f64 = 0.;
    for y in
        ((rect.y * h as f64) as u32)..(((rect.y + rect.height) * h as f64).ceil() as u32).min(h)
    {
        for x in
            ((rect.x * w as f64) as u32)..(((rect.x + rect.width) * w as f64).ceil() as u32).min(w)
        {
            for (c, v) in rgb.get_pixel(x, y).0.iter().enumerate() {
                sum[c] += *v as f64 / 255.;
            }
            count += 1.;
        }
    }
    sum.map(|v| v / count.max(1.))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn person(x: f64) -> Candidate {
        Candidate {
            rectangle: RegionRectV1 {
                x,
                y: 0.1,
                width: 0.3,
                height: 0.7,
            },
            appearance: [0.4; 3],
        }
    }
    #[test]
    fn follows_motion_but_abstains_on_crossing_cut_loss_and_gap() {
        let p = person(0.2);
        assert_eq!(
            associate(&p, &[person(0.23)], 0.9, 0.5),
            Association::Matched(0)
        );
        assert_eq!(
            associate(&p, &[person(0.22), person(0.24)], 0.9, 0.5),
            Association::Stopped("ambiguous_subjects")
        );
        assert_eq!(
            associate(&p, &[person(0.22)], 0.1, 0.5),
            Association::Stopped("scene_cut")
        );
        assert_eq!(
            associate(&p, &[], 0.9, 0.5),
            Association::Stopped("subject_lost")
        );
        assert_eq!(
            associate(&p, &[person(0.22)], 0.9, 2.),
            Association::Stopped("sampling_gap")
        );
    }
    #[test]
    fn repeated_presentation_times_do_not_drop_distinct_indexed_frames() {
        assert_eq!(
            associate(&person(0.2), &[person(0.21)], 0.9, 0.),
            Association::Matched(0)
        );
    }
    #[test]
    fn appearance_prevents_obvious_subject_swap() {
        let mut other = person(0.2);
        other.appearance = [0.9; 3];
        assert_eq!(
            associate(&person(0.2), &[other], 0.9, 0.5),
            Association::Stopped("subject_lost")
        );
    }
}
