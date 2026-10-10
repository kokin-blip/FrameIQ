use frameiq_contracts::{AppError, AppResult, IndexedFrame};
/// Presentation order is authoritative. Equal timestamp boundaries choose the
/// last frame at that boundary; an explicit ordinal still addresses every frame.
pub fn resolve_timestamp(frames: &[IndexedFrame], time: f64) -> AppResult<usize> {
    if !time.is_finite() {
        return Err(AppError::new(
            "INVALID_TIMESTAMP",
            "Enter a finite timestamp.",
        ));
    }
    if frames.is_empty() {
        return Err(AppError::new(
            "INDEX_PENDING",
            "No decoded frames have been indexed yet.",
        ));
    }
    let mut containing = None;
    for (i, f) in frames.iter().enumerate() {
        let end = f
            .duration_seconds
            .map(|d| f.time_seconds + d)
            .or_else(|| frames.get(i + 1).map(|n| n.time_seconds));
        if f.time_seconds <= time && end.is_some_and(|e| time < e) {
            containing = Some(i);
        }
        if f.time_seconds == time {
            containing = Some(i);
        }
    }
    if let Some(i) = containing {
        return Ok(i);
    }
    Ok(frames
        .iter()
        .enumerate()
        .min_by(|(a, x), (b, y)| {
            (x.time_seconds - time)
                .abs()
                .total_cmp(&(y.time_seconds - time).abs())
                .then_with(|| b.cmp(a))
        })
        .unwrap()
        .0)
}
#[cfg(test)]
mod tests {
    use super::*;
    use frameiq_contracts::TimestampOrigin;
    fn f(i: usize, t: f64, d: f64) -> IndexedFrame {
        IndexedFrame {
            ordinal: i.to_string(),
            pts_ticks: Some((t * 1000.).to_string()),
            time_seconds: t,
            duration_seconds: Some(d),
            timestamp_origin: TimestampOrigin::Source,
            key_frame: i == 0,
        }
    }
    #[test]
    fn vfr_intervals_and_boundaries() {
        let fs = vec![f(0, 0., 0.04), f(1, 0.04, 0.12), f(2, 0.16, 0.04)];
        assert_eq!(resolve_timestamp(&fs, 0.1).unwrap(), 1);
        assert_eq!(resolve_timestamp(&fs, 0.16).unwrap(), 2);
    }
    #[test]
    fn duplicate_pts_keep_ordinal_identity() {
        let fs = vec![f(0, 0., 0.04), f(1, 0., 0.04)];
        assert_eq!(resolve_timestamp(&fs, 0.).unwrap(), 1);
        assert_eq!(fs[0].ordinal, "0");
    }
    #[test]
    fn gaps_nearest_and_invalid() {
        let fs = vec![f(0, 0., 0.04), f(1, 1., 0.04)];
        assert_eq!(resolve_timestamp(&fs, 0.5).unwrap(), 1);
        assert!(resolve_timestamp(&fs, f64::NAN).is_err());
    }
}
