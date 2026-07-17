//! Calcul du facteur d'echelle applique aux deltas bruts de la souris.
//!
//! Derivation : pour un DPI (comptes par pouce) et une sensibilite in-game
//! `sens`, un deplacement de la souris de D pouces produit
//! `D * DPI` comptes, et chaque compte fait tourner la camera de
//! `deg_per_count_at_ref * (sens / reference_sensitivity)` degres (le jeu
//! est suppose lineaire en sens autour de la reference, cf. config.rs).
//!
//! Donc pour un tour complet (360 deg) :
//!   D_360_pouces = 360 / (DPI * deg_per_count_at_ref * sens/ref)
//!   cm_360_natif = 2.54 * D_360_pouces
//!
//! On veut cm_360_natif == target_cm_per_360 apres application d'un facteur
//! multiplicatif F sur les comptes bruts (equivalent a un DPI effectif de
//! DPI * F). En substituant DPI -> DPI * F et en resolvant pour F :
//!
//!   F = (2.54 * 360) / (target_cm_per_360 * DPI * deg_per_count_at_ref * sens/ref)

const CM_PER_INCH: f64 = 2.54;
const DEGREES_PER_TURN: f64 = 360.0;

/// Calcule le facteur multiplicatif a appliquer aux deltas x/y bruts.
///
/// Retourne 1.0 (passthrough) si un parametre est invalide (<=0), plutot que
/// de produire NaN/infini qui rendrait la souris inutilisable.
pub fn compute_factor(
    target_cm_per_360: f64,
    mouse_dpi: f64,
    deg_per_count_at_ref: f64,
    reference_sensitivity: f64,
    effective_sensitivity: f64,
) -> f64 {
    if target_cm_per_360 <= 0.0
        || mouse_dpi <= 0.0
        || deg_per_count_at_ref <= 0.0
        || reference_sensitivity <= 0.0
        || effective_sensitivity <= 0.0
    {
        return 1.0;
    }

    let sens_ratio = effective_sensitivity / reference_sensitivity;
    let denominator = target_cm_per_360 * mouse_dpi * deg_per_count_at_ref * sens_ratio;
    if denominator <= 0.0 || !denominator.is_finite() {
        return 1.0;
    }

    let factor = (CM_PER_INCH * DEGREES_PER_TURN) / denominator;
    if factor.is_finite() && factor > 0.0 {
        factor
    } else {
        1.0
    }
}

/// Accumulateur a resolution sous-comptage pour un axe.
///
/// Les deltas souris sont des entiers (comptes HID) ; un facteur fractionnaire
/// (ex 0.37) applique naivement avec un `round()` a chaque evenement perd de
/// la precision et introduit du jitter/drift a bas facteur. On accumule le
/// reste fractionnaire d'un evenement a l'autre pour que l'erreur moyenne
/// tende vers zero sur la duree, comme le fait un vrai capteur a DPI plus
/// bas.
#[derive(Default, Clone, Copy)]
pub struct AxisAccumulator {
    remainder: f64,
}

impl AxisAccumulator {
    pub fn apply(&mut self, raw_delta: i32, factor: f64) -> i32 {
        let scaled = raw_delta as f64 * factor + self.remainder;
        let out = scaled.trunc();
        self.remainder = scaled - out;
        out as i32
    }

    pub fn reset(&mut self) {
        self.remainder = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factor_is_one_when_target_matches_native() {
        // A DPI=800, deg_per_count=0.022 (~CS-like), reference_sensitivity=1,
        // le cm/360 natif est 2.54*360/(800*0.022) = 51.95cm environ.
        let native_cm360 = (CM_PER_INCH * DEGREES_PER_TURN) / (800.0 * 0.022);
        let f = compute_factor(native_cm360, 800.0, 0.022, 1.0, 1.0);
        assert!((f - 1.0).abs() < 1e-9);
    }

    #[test]
    fn factor_scales_inversely_with_target() {
        let f1 = compute_factor(30.0, 800.0, 0.022, 1.0, 1.0);
        let f2 = compute_factor(60.0, 800.0, 0.022, 1.0, 1.0);
        assert!((f1 - 2.0 * f2).abs() < 1e-9);
    }

    #[test]
    fn invalid_inputs_fall_back_to_passthrough() {
        assert_eq!(compute_factor(0.0, 800.0, 0.022, 1.0, 1.0), 1.0);
        assert_eq!(compute_factor(30.0, -1.0, 0.022, 1.0, 1.0), 1.0);
    }

    #[test]
    fn accumulator_preserves_fractional_precision_over_time() {
        let mut acc = AxisAccumulator::default();
        let factor = 0.37;
        let mut exact_total = 0.0f64;
        let mut applied_total = 0i32;
        for _ in 0..1000 {
            exact_total += 5.0 * factor;
            applied_total += acc.apply(5, factor);
        }
        // L'erreur cumulee doit rester bornee (< 1 compte), pas croitre avec le temps.
        assert!((exact_total - applied_total as f64).abs() < 1.0);
    }
}
