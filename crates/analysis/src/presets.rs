use frameiq_contracts::*;
pub fn builtins() -> Vec<PresetV1> {
    [
  ("makeup","Makeup Detail",[30.,20.,30.,10.,10.],vec!["face_landmarks"]),
  ("hairstyle","Hairstyle Detail",[25.,15.,30.,20.,10.],vec!["head_region","hair_mask"]),
  ("outfit","Outfit / Fashion",[20.,15.,25.,30.,10.],vec!["person","pose","person_mask"]),
  ("complete","Complete Look",[20.,15.,25.,25.,15.],vec!["person","pose","view_orientation"]),
  ("accessories","Accessories",[30.,15.,30.,15.,10.],vec!["accessory_region"]),
  ("footwear","Footwear",[25.,15.,30.,20.,10.],vec!["foot_landmarks","footwear_region"]),
  ("custom","Custom Region",[35.,20.,30.,5.,10.],vec!["region_tracking"]),
 ].into_iter().map(|(id,name,weights,required)|PresetV1{id:id.into(),name:name.into(),version:1,weights,count:8,required_capabilities:required.into_iter().map(String::from).collect(),automatic_available:false,unavailable_reason:Some("Automatic eligibility is not yet validated. Use guided regions or experimental tracking for supported presets.".into())}).collect()
}
pub fn validate(p: &PresetV1) -> AppResult<()> {
    if p.name.trim().is_empty()
        || p.name.len() > 120
        || !(1..=50).contains(&p.count)
        || p.weights.iter().any(|n| !n.is_finite() || *n < 0.)
        || p.weights.iter().sum::<f64>() <= 0.
    {
        return Err(AppError::new(
            "INVALID_PRESET",
            "Choose a name, 1–50 results, and finite nonnegative weights with a positive sum.",
        ));
    }
    Ok(())
}
pub fn validate_rectangle(r: &RegionRectV1) -> AppResult<()> {
    if [r.x, r.y, r.width, r.height].iter().any(|n| !n.is_finite())
        || r.x < 0.
        || r.y < 0.
        || r.width <= 0.
        || r.height <= 0.
        || r.x + r.width > 1. + 1e-9
        || r.y + r.height > 1. + 1e-9
    {
        return Err(AppError::new(
            "INVALID_REGION",
            "Draw a positive rectangle inside the image.",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preset_weights_and_missing_capabilities_are_explicit() {
        let presets = builtins();
        assert_eq!(presets.len(), 7);
        for p in presets {
            validate(&p).unwrap();
            assert_eq!(p.weights.iter().sum::<f64>(), 100.);
            assert!(!p.automatic_available);
            assert!(p.unavailable_reason.is_some());
        }
    }
    #[test]
    fn regions_cannot_escape_the_image() {
        assert!(validate_rectangle(&RegionRectV1 {
            x: 0.9,
            y: 0.,
            width: 0.2,
            height: 0.1
        })
        .is_err());
        assert!(validate_rectangle(&RegionRectV1 {
            x: 0.,
            y: 0.,
            width: 0.5,
            height: 0.5
        })
        .is_ok());
    }
}
