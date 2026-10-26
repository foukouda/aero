//! International Standard Atmosphere (ISA), as tabulated in the U.S.
//! Standard Atmosphere 1976 up to 20 km: a troposphere whose temperature
//! falls by 6.5 K per kilometre, then an isothermal layer at 216.65 K.

use serde::{Deserialize, Serialize};

use crate::error::{MissionError, Result};

/// Standard gravity, m/s².
pub const G0: f64 = 9.806_65;
/// Specific gas constant of air, J/(kg·K).
pub const R_AIR: f64 = 287.052_87;
/// Ratio of specific heats of air.
pub const GAMMA: f64 = 1.4;
/// Sea-level temperature, K.
pub const T0: f64 = 288.15;
/// Sea-level pressure, Pa.
pub const P0: f64 = 101_325.0;
/// Lowest altitude of the model, m.
pub const MIN_ALTITUDE: f64 = -1_000.0;
/// Highest altitude of the model, m.
pub const MAX_ALTITUDE: f64 = 20_000.0;
/// Largest accepted deviation from the standard temperature, K.
pub const MAX_TEMPERATURE_OFFSET: f64 = 60.0;

/// Effective Earth radius of the geopotential altitude, m.
const EARTH_RADIUS: f64 = 6_356_766.0;
/// Temperature lapse rate of the troposphere, K/m.
const LAPSE_RATE: f64 = 0.0065;
/// Geopotential altitude of the tropopause, m.
const TROPOPAUSE: f64 = 11_000.0;
/// Sutherland's law: μ = β T^1.5 / (T + S).
const SUTHERLAND_BETA: f64 = 1.458e-6;
const SUTHERLAND_S: f64 = 110.4;

/// State of the air at an altitude.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Atmosphere {
    /// Geometric altitude, m.
    pub altitude: f64,
    /// Temperature, K.
    pub temperature: f64,
    /// Pressure, Pa.
    pub pressure: f64,
    /// Density ρ, kg/m³.
    pub density: f64,
    /// Dynamic viscosity μ, Pa·s.
    pub dynamic_viscosity: f64,
    /// Kinematic viscosity ν = μ/ρ, m²/s.
    pub kinematic_viscosity: f64,
    /// Speed of sound, m/s.
    pub speed_of_sound: f64,
}

/// The standard atmosphere at a geometric `altitude` (m, from −1 000 to
/// 20 000), with the temperature shifted by `temperature_offset` (K; a hot
/// day is "ISA + 15"). The pressure is the standard one at that altitude,
/// the offset changes the density, the viscosity and the speed of sound.
pub fn isa(altitude: f64, temperature_offset: f64) -> Result<Atmosphere> {
    if !(MIN_ALTITUDE..=MAX_ALTITUDE).contains(&altitude) {
        return Err(MissionError::InvalidInput(format!(
            "altitude must be between {MIN_ALTITUDE} and {MAX_ALTITUDE} m, got {altitude}"
        )));
    }
    if !(-MAX_TEMPERATURE_OFFSET..=MAX_TEMPERATURE_OFFSET).contains(&temperature_offset) {
        return Err(MissionError::InvalidInput(format!(
            "temperature offset must be within ±{MAX_TEMPERATURE_OFFSET} K, got {temperature_offset}"
        )));
    }
    // The layers are defined on the geopotential altitude.
    let h = EARTH_RADIUS * altitude / (EARTH_RADIUS + altitude);
    let exponent = G0 / (R_AIR * LAPSE_RATE);
    let (standard_temperature, pressure) = if h <= TROPOPAUSE {
        let t = T0 - LAPSE_RATE * h;
        (t, P0 * (t / T0).powf(exponent))
    } else {
        let t = T0 - LAPSE_RATE * TROPOPAUSE;
        let p = P0 * (t / T0).powf(exponent);
        (t, p * (-G0 * (h - TROPOPAUSE) / (R_AIR * t)).exp())
    };
    let temperature = standard_temperature + temperature_offset;
    let density = pressure / (R_AIR * temperature);
    let dynamic_viscosity = SUTHERLAND_BETA * temperature.powf(1.5) / (temperature + SUTHERLAND_S);
    Ok(Atmosphere {
        altitude,
        temperature,
        pressure,
        density,
        dynamic_viscosity,
        kinematic_viscosity: dynamic_viscosity / density,
        speed_of_sound: (GAMMA * R_AIR * temperature).sqrt(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// U.S. Standard Atmosphere 1976 by geometric altitude, as printed in
    /// the tables of Public Domain Aeronautical Software
    /// (pdas.com/bigtables.html): altitude, T, p, ρ, a, μ, five
    /// significant digits.
    const TABLE: [[f64; 6]; 5] = [
        [0.0, 288.150, 1.0132e5, 1.2250, 340.29, 1.7894e-5],
        [5_000.0, 255.676, 5.4048e4, 7.3643e-1, 320.55, 1.6282e-5],
        [10_000.0, 223.252, 2.6500e4, 4.1351e-1, 299.53, 1.4577e-5],
        [15_000.0, 216.650, 1.2112e4, 1.9476e-1, 295.07, 1.4216e-5],
        [20_000.0, 216.650, 5.5293e3, 8.8910e-2, 295.07, 1.4216e-5],
    ];

    fn assert_table(value: f64, table: f64, what: &str, altitude: f64) {
        // Half a unit of the fifth significant digit, and a little slack.
        let tolerance = 6e-5 * table.abs();
        assert!(
            (value - table).abs() <= tolerance,
            "{what} at {altitude} m: {value} vs table {table}"
        );
    }

    #[test]
    fn matches_the_1976_table() {
        for [z, t, p, rho, a, mu] in TABLE {
            let air = isa(z, 0.0).unwrap();
            assert!(
                (air.temperature - t).abs() < 1e-3,
                "T at {z} m: {}",
                air.temperature
            );
            assert_table(air.pressure, p, "p", z);
            assert_table(air.density, rho, "rho", z);
            assert_table(air.speed_of_sound, a, "a", z);
            assert_table(air.dynamic_viscosity, mu, "mu", z);
            assert!((air.kinematic_viscosity - mu / rho).abs() < 1e-4 * mu / rho);
        }
    }

    #[test]
    fn continuous_at_the_tropopause() {
        // Geometric altitude of the 11 km geopotential tropopause.
        let z = EARTH_RADIUS * TROPOPAUSE / (EARTH_RADIUS - TROPOPAUSE);
        // 2 µm apart: the pressure differs by ρ g δz ≈ 7e-6 Pa.
        let below = isa(z - 1e-6, 0.0).unwrap();
        let above = isa(z + 1e-6, 0.0).unwrap();
        assert!((below.pressure - above.pressure).abs() < 1e-4);
        assert!((below.temperature - above.temperature).abs() < 1e-8);
    }

    #[test]
    fn hot_day_keeps_the_pressure_and_thins_the_air() {
        let standard = isa(100.0, 0.0).unwrap();
        let hot = isa(100.0, 15.0).unwrap();
        assert_eq!(hot.pressure, standard.pressure);
        assert!((hot.temperature - standard.temperature - 15.0).abs() < 1e-12);
        assert!(
            (hot.density * hot.temperature - standard.density * standard.temperature).abs() < 1e-9
        );
        assert!(hot.kinematic_viscosity > standard.kinematic_viscosity);
    }

    #[test]
    fn rejects_out_of_range_inputs() {
        assert!(isa(-1_001.0, 0.0).is_err());
        assert!(isa(20_001.0, 0.0).is_err());
        assert!(isa(f64::NAN, 0.0).is_err());
        assert!(isa(0.0, 61.0).is_err());
        assert!(isa(0.0, f64::NAN).is_err());
        assert!(isa(-1_000.0, -60.0).is_ok());
    }
}
